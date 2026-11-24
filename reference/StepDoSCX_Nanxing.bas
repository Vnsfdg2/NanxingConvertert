Attribute VB_Name = "StepDoSCX_Nanxing"
Option Explicit

' ==============================================================================
' MAKRO ALPHACAM VBA: AUTOMATYCZNY IMPORT STEP -> EKSTRAKCJA WIERCEN -> SCX
' Przeznaczenie: Generowanie programow SCX na wiertarke 6-stronna NANXING
' Obslugiwane operacje: Wiercenie pionowe (gora Face 5, dol Face 6)
'                       oraz wiercenie poziome (boki Face 1, 2, 3, 4)
' ==============================================================================

#If VBA7 Then
    Private Declare PtrSafe Function GetOpenFileNameW Lib "comdlg32.dll" (ByRef pOpenfilename As OPENFILENAMEW) As Long
    Private Type OPENFILENAMEW
        lStructSize As Long
        hwndOwner As LongPtr
        hInstance As LongPtr
        lpstrFilter As LongPtr
        lpstrCustomFilter As LongPtr
        nMaxCustFilter As Long
        nFilterIndex As Long
        lpstrFile As LongPtr
        nMaxFile As Long
        lpstrFileTitle As LongPtr
        nMaxFileTitle As Long
        lpstrInitialDir As LongPtr
        lpstrTitle As LongPtr
        flags As Long
        nFileOffset As Integer
        nFileExtension As Integer
        lpstrDefExt As LongPtr
        lCustData As LongPtr
        lpfnHook As LongPtr
        lpTemplateName As LongPtr
        pvReserved As LongPtr
        dwReserved As Long
        flagsEx As Long
    End Type
#Else
    Private Declare Function GetOpenFileNameW Lib "comdlg32.dll" (ByRef pOpenfilename As OPENFILENAMEW) As Long
    Private Type OPENFILENAMEW
        lStructSize As Long
        hwndOwner As Long
        hInstance As Long
        lpstrFilter As Long
        lpstrCustomFilter As Long
        nMaxCustFilter As Long
        nFilterIndex As Long
        lpstrFile As Long
        nMaxFile As Long
        lpstrFileTitle As Long
        nMaxFileTitle As Long
        lpstrInitialDir As Long
        lpstrTitle As Long
        flags As Long
        nFileOffset As Integer
        nFileExtension As Integer
        lpstrDefExt As Long
        lCustData As Long
        lpfnHook As Long
        lpTemplateName As Long
        pvReserved As Long
        dwReserved As Long
        flagsEx As Long
    End Type
#End If

' --- STALE ALPHACAM FEATURE EXTRACTION ---
Private Const FEAT_IMPORT_VISIBLE_SOLID As Long = 0
Private Const FEAT_FILE_STEP As Long = 11
Private Const FEAT_ALIGN_MIN_EXTENT As Long = 0
Private Const FEAT_ALIGN_MAX_EXTENT As Long = 2
Private Const FEAT_ALL_BODIES_EXTRACTED As Long = 0

' Orientacje ekstrakcji otworow:
Private Const FEAT_ORIENT_FLAT_LAND As Long = 0
Private Const FEAT_ORIENT_CURRENT_PLANE As Long = 1
Private Const FEAT_ORIENT_ANY As Long = 2
Private Const FEAT_ORIENT_ORTHOGONAL As Long = 3

' Maska scianek prostopadlych dla WorkPlanes (63 = wszystkie 6 scian: gora, dol, 4 boki)
Private Const FEAT_WP_ALL_FACES As Long = 63

' Atrybuty wewnetrzne Alphacam
Private Const ATTR_GEO_Z_LEVEL_TOP As String = "LicomUKDMBGeoZLevelTop"
Private Const ATTR_GEO_Z_LEVEL_BOTTOM As String = "LicomUKDMBGeoZLevelBottom"
Private Const ATTR_HOLE_DIAMETER As String = "LicomUKRJMHoleDiameter"

' --- STALE WIERCENIA NANXING SCX ---
Private Const MAX_GLEBOKOSC_WIERCENIA_BOCZNEGO As Double = 50#
Private Const NADDANEK_OTWORU_POZIOMEGO_BEZ_DNA As Double = 2#

' --- STALE GNIAZDA LAMELLO P14 (NANXING SCX) ---
Private Const LAMELLO_P14_LENGTH As Double = 66#
Private Const LAMELLO_P14_WIDTH_GORA As Double = 9.8
Private Const LAMELLO_P14_WIDTH_BRZEG As Double = 10.15
Private Const LAMELLO_P14_MAX_DEPTH As Double = 14.1
Private Const LAMELLO_P14_IMBUS_DIA As Double = 6#
Private Const LAMELLO_P14_IMBUS_OFFSET As Double = 7.5

' --- TOLERANCJE FILTROWANIA WYCIECIA LAMELLO P14 Z MODELU STEP ---
' Promien tarczy w modelu STEP: ~50.199 mm (srednica ~100.4 mm)
Private Const LAMELLO_STEP_RAD_MIN As Double = 47#
Private Const LAMELLO_STEP_RAD_MAX As Double = 53.5
' Dlugosc wyciecia (cieciwy) na powierzchni wejsciowej w STEP: ~69.56 mm (lub nominalnie 66 mm)
Private Const LAMELLO_STEP_LEN_MIN As Double = 63#
Private Const LAMELLO_STEP_LEN_MAX As Double = 73.5
' Szerokosc wyciecia w STEP: ~10.0 mm (9.8 .. 10.15 mm)
Private Const LAMELLO_STEP_WID_MIN As Double = 8.5
Private Const LAMELLO_STEP_WID_MAX As Double = 11.5
' Glebokosc wyciecia w STEP: ~14.0 mm (14.1 mm)
Private Const LAMELLO_STEP_DEP_MIN As Double = 11.5
Private Const LAMELLO_STEP_DEP_MAX As Double = 16.5

Private Type DaneGniazdaLamelloP14
    face As Long           ' 1=Tyl(Y=W), 2=Przod(Y=0), 3=Prawo(X=L), 4=Lewo(X=0), 5=Gora(Z=T), 6=Dol(Z=0)
    kierunek As String     ' "X" lub "Y"
    x As Double            ' Wspolrzedna X srodka gniazda
    y As Double            ' Wspolrzedna Y srodka gniazda
    z As Double            ' Wspolrzedna Z (dla Face 5 -> T, dla Face 6 -> 0, dla Face 1..4 -> srodek wysokosci gniazda)
    czyBrzegowe As Boolean ' True dla scian bocznych Face 1..4 (wymaga otworu technicznego fi 6 na Face 5)
End Type

' --- ZAPAMIETANE USTAWIENIA W BIEZACEJ SESJI ---
Private g_OstatniaSciezkaStep As String
Private g_OstatniBlad As String
Private g_InicjalizowanoUstawienia As Boolean
Private g_ZamienGzymki As Boolean
Private g_ZapisujWPodfolderze As Boolean
Private g_ZerujKolorBialy As Boolean
Private g_KompensujStozekWiertla As Boolean
Private g_UwzgledniajCzescioweOtwory As Boolean

' ==============================================================================
' INICJALIZACJA DOMYSLNYCH USTAWIEN (RAZ NA SESJE)
' ==============================================================================
Private Sub InicjalizujUstawieniaDomyslne()
    If g_InicjalizowanoUstawienia Then Exit Sub
    g_ZamienGzymki = True
    g_ZapisujWPodfolderze = True
    g_ZerujKolorBialy = True
    g_KompensujStozekWiertla = True
    g_UwzgledniajCzescioweOtwory = False
    If Trim$(g_OstatniaSciezkaStep) = "" Then
        g_OstatniaSciezkaStep = PobierzUniwersalnyFolderStartowy()
    End If
    g_InicjalizowanoUstawienia = True
End Sub

' ==============================================================================
' UNIWERSALNY FOLDER STARTOWY (DZIALA NA KAZDYM PC Z WINDOWS 10 / 11)
' Bezpieczne na nowym PC - pierwszy raz otwiera Pulpit/Dokumenty,
' a potem zapamietuje ostatnia lokalizacje w biezacej sesji.
' ==============================================================================
Private Function PobierzUniwersalnyFolderStartowy() As String
    On Error Resume Next
    Dim fso As Object
    Set fso = CreateObject("Scripting.FileSystemObject")

    ' 1. Jesli w biezacej sesji wybrano juz istniejacy folder lub plik, otwieramy go ponownie
    If Trim$(g_OstatniaSciezkaStep) <> "" Then
        If fso.FolderExists(g_OstatniaSciezkaStep) Then
            PobierzUniwersalnyFolderStartowy = g_OstatniaSciezkaStep
            Exit Function
        ElseIf fso.FileExists(g_OstatniaSciezkaStep) Then
            PobierzUniwersalnyFolderStartowy = fso.GetParentFolderName(g_OstatniaSciezkaStep)
            Exit Function
        End If
    End If

    ' 2. Pulpit lub Moje Dokumenty uzytkownika (zawsze obecne na kazdym Windows 10 i 11)
    Dim wsh As Object
    Set wsh = CreateObject("WScript.Shell")
    Dim sciezka As String
    If Not (wsh Is Nothing) Then
        sciezka = Trim$(CStr(wsh.SpecialFolders("Desktop")))
        If sciezka <> "" And fso.FolderExists(sciezka) Then
            PobierzUniwersalnyFolderStartowy = sciezka
            Exit Function
        End If
        sciezka = Trim$(CStr(wsh.SpecialFolders("MyDocuments")))
        If sciezka <> "" And fso.FolderExists(sciezka) Then
            PobierzUniwersalnyFolderStartowy = sciezka
            Exit Function
        End If
    End If

    ' 3. Folder profilu uzytkownika (%USERPROFILE%)
    sciezka = Trim$(Environ$("USERPROFILE"))
    If sciezka <> "" And fso.FolderExists(sciezka) Then
        PobierzUniwersalnyFolderStartowy = sciezka
        Exit Function
    End If

    ' 4. Dysk systemowy (np. C:\)
    sciezka = Trim$(Environ$("SystemDrive"))
    If sciezka <> "" Then
        sciezka = sciezka & "\"
        If fso.FolderExists(sciezka) Then
            PobierzUniwersalnyFolderStartowy = sciezka
            Exit Function
        End If
    End If

    PobierzUniwersalnyFolderStartowy = "C:\"
    On Error GoTo 0
End Function

' ==============================================================================
' GLOWNE PUNKTY WEJSCIA Z WSTAZKI CAD / ADD-INS
' ==============================================================================
Public Sub UruchomMenuStepDoSCX()
    InicjalizujUstawieniaDomyslne

    Dim trybAkcji As String
    If Not PokazOknoMenuStepDoScx(trybAkcji) Then
        Exit Sub
    End If

    If UCase$(trybAkcji) = "SINGLE" Then
        WykonajDlaPojedynczegoPliku
    ElseIf UCase$(trybAkcji) = "FOLDER" Then
        WykonajDlaCalegoFolderu
    End If
End Sub

Public Sub KonwertujPojedynczyStepDoSCX()
    UruchomMenuStepDoSCX
End Sub

Public Sub KonwertujFolderStepDoSCX()
    UruchomMenuStepDoSCX
End Sub

' ==============================================================================
' 1. WYKONANIE DLA POJEDYNCZEGO PLIKU STEP
' ==============================================================================
Private Sub WykonajDlaPojedynczegoPliku()
    Dim sciezkaStep As String
    Dim sciezkaScx As String
    Dim folderPliku As String
    Dim fso As Object

    sciezkaStep = WybierzPlikStep_Dialog(g_OstatniaSciezkaStep, "Wybierz pojedynczy plik .STEP / .STP do konwersji na SCX (Nanxing)")
    sciezkaStep = Trim$(Replace(sciezkaStep, """", ""))
    If sciezkaStep = "" Then Exit Sub

    If Dir$(sciezkaStep) = "" Then
        MsgBox "Nie znaleziono pliku:" & vbCrLf & sciezkaStep, vbExclamation, "Blad"
        Exit Sub
    End If

    g_OstatniaSciezkaStep = sciezkaStep
    Set fso = CreateObject("Scripting.FileSystemObject")
    folderPliku = fso.GetParentFolderName(sciezkaStep)
    If Right$(folderPliku, 1) <> "\" Then folderPliku = folderPliku & "\"

    If g_ZapisujWPodfolderze Then
        Dim folderScx As String
        folderScx = folderPliku & "SCX_NANXING\"
        sciezkaScx = folderScx & fso.GetBaseName(sciezkaStep) & ".scx"
    Else
        sciezkaScx = folderPliku & fso.GetBaseName(sciezkaStep) & ".scx"
    End If

    Dim ileOtworow As Long
    Dim ileLamello As Long
    Dim pominietoBrakOperacji As Boolean
    pominietoBrakOperacji = False
    If PrzetworzPlikStepDoScx(sciezkaStep, sciezkaScx, g_ZamienGzymki, g_ZerujKolorBialy, g_KompensujStozekWiertla, ileOtworow, ileLamello, pominietoBrakOperacji) Then
        If pominietoBrakOperacji Then
            MsgBox "Wybrana formatka STEP nie posiada zadnych operacji wiercenia ani gniazd Lamello P14:" & vbCrLf & _
                   sciezkaStep & vbCrLf & vbCrLf & _
                   "Pominieto tworzenie pliku programu SCX dla tej formatki.", _
                   vbInformation, "Brak operacji - STEP -> SCX (Nanxing)"
        Else
            MsgBox "Pomyslnie wygenerowano program SCX dla Nanxing:" & vbCrLf & _
                   sciezkaScx & vbCrLf & vbCrLf & _
                   "Liczba wyekstrahowanych otworow: " & ileOtworow & vbCrLf & _
                   "Liczba gniazd Lamello P14: " & ileLamello, _
                   vbInformation, "Gotowe - STEP -> SCX (Nanxing)"
        End If
    Else
        MsgBox "Wystapil blad podczas przetwarzania pliku:" & vbCrLf & sciezkaStep & _
               vbCrLf & vbCrLf & "Szczegoly: " & g_OstatniBlad, _
               vbCritical, "Blad STEP -> SCX"
    End If
End Sub

' ==============================================================================
' 2. WYKONANIE DLA CALEGO FOLDERU STEP
' ==============================================================================
Private Sub WykonajDlaCalegoFolderu()
    Dim wskazanyPlik As String
    Dim folderWejsciowy As String
    Dim folderWyjsciowy As String
    Dim plik As String
    Dim licznik As Long
    Dim pominiete As Long
    Dim bledy As Long
    Dim ostatniBladFolderu As String
    Dim sumaOtworow As Long
    Dim sumaLamello As Long
    Dim fso As Object

    wskazanyPlik = WybierzPlikStep_Dialog(g_OstatniaSciezkaStep, _
        "Wskaz dowolny plik .STEP / .STP z folderu (przekonwertowany zostanie CALY FOLDER)")
    wskazanyPlik = Trim$(Replace(wskazanyPlik, """", ""))
    If wskazanyPlik = "" Then Exit Sub

    Set fso = CreateObject("Scripting.FileSystemObject")
    If fso.FolderExists(wskazanyPlik) Then
        folderWejsciowy = wskazanyPlik
    Else
        folderWejsciowy = fso.GetParentFolderName(wskazanyPlik)
    End If

    If Trim$(folderWejsciowy) = "" Then Exit Sub
    If Right$(folderWejsciowy, 1) <> "\" Then folderWejsciowy = folderWejsciowy & "\"
    g_OstatniaSciezkaStep = wskazanyPlik

    If g_ZapisujWPodfolderze Then
        folderWyjsciowy = folderWejsciowy & "SCX_NANXING\"
    Else
        folderWyjsciowy = folderWejsciowy
    End If

    Dim listaPlikow As New Collection
    plik = Dir$(folderWejsciowy & "*.*")
    Do While plik <> ""
        Dim ext As String
        ext = LCase$(fso.GetExtensionName(plik))
        If ext = "stp" Or ext = "step" Then
            listaPlikow.Add plik
        End If
        plik = Dir$()
    Loop

    If listaPlikow.Count = 0 Then
        MsgBox "W wybranym folderze nie znaleziono zadnych plikow .stp / .step:" & vbCrLf & folderWejsciowy, _
               vbExclamation, "Brak plikow STEP"
        Exit Sub
    End If

    licznik = 0
    pominiete = 0
    bledy = 0
    sumaOtworow = 0
    sumaLamello = 0
    ostatniBladFolderu = ""

    Dim idx As Long
    For idx = 1 To listaPlikow.Count
        Dim nazwaPliku As String
        Dim pelnyStep As String
        Dim pelnyScx As String
        Dim otwPlik As Long
        Dim lamPlik As Long
        Dim pominietoPlik As Boolean
        nazwaPliku = CStr(listaPlikow(idx))
        pelnyStep = folderWejsciowy & nazwaPliku
        pelnyScx = folderWyjsciowy & fso.GetBaseName(nazwaPliku) & ".scx"

        otwPlik = 0
        lamPlik = 0
        pominietoPlik = False
        If PrzetworzPlikStepDoScx(pelnyStep, pelnyScx, g_ZamienGzymki, g_ZerujKolorBialy, g_KompensujStozekWiertla, otwPlik, lamPlik, pominietoPlik) Then
            If pominietoPlik Then
                pominiete = pominiete + 1
            Else
                licznik = licznik + 1
                sumaOtworow = sumaOtworow + otwPlik
                sumaLamello = sumaLamello + lamPlik
            End If
        Else
            bledy = bledy + 1
            ostatniBladFolderu = nazwaPliku & ": " & g_OstatniBlad
        End If
    Next idx

    Dim komunikat As String
    komunikat = "Zakonczono konwersje folderu dla wiertarki Nanxing!" & vbCrLf & _
                "Utworzono programow SCX: " & licznik & " (z " & listaPlikow.Count & " plikow STEP)" & vbCrLf & _
                "Pominieto formatek bez operacji: " & pominiete & vbCrLf & _
                "Laczna liczba wygenerowanych otworow: " & sumaOtworow & vbCrLf & _
                "Laczna liczba gniazd Lamello P14: " & sumaLamello & vbCrLf & _
                "Folder zapisu: " & folderWyjsciowy
    If bledy > 0 Then
        komunikat = komunikat & vbCrLf & vbCrLf & "Bledy (" & bledy & "): " & ostatniBladFolderu
    End If

    MsgBox komunikat, vbInformation, "Gotowe - STEP -> SCX (Nanxing)"
End Sub

' ==============================================================================
' GLOWNA FUNKCJA PRZETWARZAJACA STEP -> SCX
' ==============================================================================
Private Function PrzetworzPlikStepDoScx( _
    ByVal sciezkaStep As String, _
    ByVal sciezkaScx As String, _
    ByVal zamienGzymki As Boolean, _
    ByVal zerujKolor As Boolean, _
    ByVal kompensujStozek As Boolean, _
    ByRef outLiczbaOtworow As Long, _
    Optional ByRef outLiczbaLamello As Long = 0, _
    Optional ByRef outPominietoBrakOperacji As Boolean = False) As Boolean

    InicjalizujUstawieniaDomyslne
    outLiczbaOtworow = 0
    outLiczbaLamello = 0
    outPominietoBrakOperacji = False
    g_OstatniBlad = ""

    Dim etapOpis As String
    On Error GoTo BladPrzetwarzania

    ' 1. Konfiguracja translatora Solid i import STEP
    etapOpis = "Import pliku STEP"
    On Error Resume Next
    Dim wshReg As Object
    Set wshReg = CreateObject("WScript.Shell")
    wshReg.RegWrite "HKCU\SOFTWARE\Hexagon\ALPHACAM\SolidImport\SolidLinkOptions\Healing", 2, "REG_DWORD"
    wshReg.RegWrite "HKCU\SOFTWARE\Hexagon\ALPHACAM\SolidImport\SolidLinkOptions\WireBodies", 0, "REG_DWORD"
    wshReg.RegWrite "HKCU\SOFTWARE\Hexagon\ALPHACAM\Feature\ExtractDlg\OrientationDrillHole", 3, "REG_DWORD"
    wshReg.RegWrite "HKCU\SOFTWARE\Hexagon\ALPHACAM\Feature\ExtractDlg\FindDrillHoles", 1, "REG_DWORD"
    wshReg.RegWrite "HKCU\SOFTWARE\Hexagon\ALPHACAM\Feature\ExtractDlg\SourceDrillHole", 0, "REG_DWORD"
    wshReg.RegWrite "HKCU\SOFTWARE\Hexagon\ALPHACAM\Feature\OrthFaceAdvOpt\ExtractDrillHoleFace", 63, "REG_DWORD"
    wshReg.RegWrite "HKCU\SOFTWARE\Hexagon\ALPHACAM\Feature\OrthFaceAdvOpt\RemoveDuplicateDrillHole", 1, "REG_DWORD"
    App.CadInputSettings.STEPTranslator = 1
    On Error GoTo BladPrzetwarzania

    App.New

    Dim drw As Drawing
    Set drw = App.ActiveDrawing
    drw.CancelWorkPlane

    Dim sf As Object
    Set sf = drw.SolidInterface

    On Error Resume Next
    If Not (sf Is Nothing) Then
        sf.AlignOnImport = False
        sf.AlignTopWithOrigin = True
        sf.AlignLongestEdgeWithXAxis = True
        sf.UseLatheAlign = False
        sf.AlignDatumX = FEAT_ALIGN_MIN_EXTENT
        sf.AlignDatumY = FEAT_ALIGN_MIN_EXTENT
        sf.AlignDatumZ = FEAT_ALIGN_MAX_EXTENT
    End If
    On Error GoTo BladPrzetwarzania

    On Error Resume Next
    App.OpenExternalFile sciezkaStep, "STEP", True
    Set drw = App.ActiveDrawing
    Set sf = drw.SolidInterface

    If LiczbaBrylWRysunku(drw, sf) = 0 Then
        App.OpenExternalFile sciezkaStep, "", True
        Set drw = App.ActiveDrawing
        Set sf = drw.SolidInterface
    End If

    If LiczbaBrylWRysunku(drw, sf) = 0 Then
        App.OpenStepFileEx sciezkaStep, True, 1
        Set drw = App.ActiveDrawing
        Set sf = drw.SolidInterface
    End If

    If LiczbaBrylWRysunku(drw, sf) = 0 And Not (sf Is Nothing) Then
        sf.Import sciezkaStep, FEAT_IMPORT_VISIBLE_SOLID
        If LiczbaBrylWRysunku(drw, sf) = 0 Then
            sf.ImportAs FEAT_FILE_STEP, sciezkaStep, FEAT_IMPORT_VISIBLE_SOLID
        End If
    End If
    On Error GoTo BladPrzetwarzania

    If LiczbaBrylWRysunku(drw, sf) = 0 Then
        g_OstatniBlad = "Translator STEP nie utworzyl zadnej bryly Solid z pliku: " & sciezkaStep
        PrzetworzPlikStepDoScx = False
        Exit Function
    End If

    If zerujKolor Then
        WyzerujKolorBrylyNaBialy drw, sf
    End If

    ' 2. AutoAlign bryly (ulozenie na plasko na stole XY)
    etapOpis = "AutoAlign bryly"
    If Not (sf Is Nothing) Then
        If sf.Bodies.Count > 0 Then
            sf.Bodies.AutoAlign
        End If
    End If

    Dim minX As Double, minY As Double, minZ As Double
    Dim maxX As Double, maxY As Double, maxZ As Double
    PobierzGabarytyBryly drw, sf, minX, maxX, minY, maxY, minZ, maxZ

    ' Jesli gniazda powierzchniowe Lamello P14 (lub jedyne obrobki nieprzelotowe) znalazly sie na spodzie (minZ),
    ' obracamy bryle o 180 stopni wokol osi X, aby powierzchnia obrabiana byla od gory (Face 5, Z = T)
    If CzyBrylaWymagaObrotuGoraDol(sf, minX, maxX, minY, maxY, minZ, maxZ) Then
        ObrocWszystkieBrylyWokolX drw, sf, 180#
        PobierzGabarytyBryly drw, sf, minX, maxX, minY, maxY, minZ, maxZ
    End If

    ' 3. Dosuniecie do (0,0) oraz SPOD formatki na Z=0 (SCX wymaga Z=0 od spodu formatki)
    etapOpis = "Ustalanie bazy (0,0) i spodu Z=0"
    If Abs(minX) > 0.0001 Or Abs(minY) > 0.0001 Or Abs(minZ) > 0.0001 Then
        PrzesunWszystkieBryly drw, sf, -minX, -minY, -minZ
        PobierzGabarytyBryly drw, sf, minX, maxX, minY, maxY, minZ, maxZ
    End If

    Dim L As Double, W As Double, T As Double
    L = maxX - minX
    W = maxY - minY
    T = maxZ - minZ

    If L <= 1# Or W <= 1# Or T <= 0.5 Then
        g_OstatniBlad = "Nieprawidlowe gabaryty formatki po imporcie STEP: L=" & L & ", W=" & W & ", T=" & T
        PrzetworzPlikStepDoScx = False
        Exit Function
    End If

    ' 4. Ekstrakcja otworow 3D na wszystkich 6 scianach bryly (Gora, Dol, 4 Boki)
    etapOpis = "Ekstrakcja otworow 3D na wszystkich sciankach"
    drw.CancelWorkPlane

    If Not (sf Is Nothing) Then
        If sf.Bodies.Count > 0 Then
            Dim ext As Object
            Dim extractionSukces As Boolean
            extractionSukces = False

            ' Proba 1: Nowoczesne API SolidExtraction2 z pelna obsluga 6 scian formatki
            On Error Resume Next
            Set ext = sf.SolidExtraction2
            If Not (ext Is Nothing) Then
                ext.Orientation = FEAT_ORIENT_ORTHOGONAL ' 3: OrthogonalFaceOrientation2
                ext.Source = FEAT_ALL_BODIES_EXTRACTED
                ext.FindContours = True
                ext.FindDrillHoles = True
                ext.ExtractContourOnWP = FEAT_WP_ALL_FACES ' 63: wszystkie 6 plaszczyzn
                ext.ExtractDrillableHoleOnWP = FEAT_WP_ALL_FACES ' 63: wszystkie 6 plaszczyzn
                ext.RemoveDuplicateContourOnWP = 1
                ext.RemoveDuplicateDrillableHoleOnWP = 1
                ext.MaxDrill = 150#
                ext.IncludePartialHoles = g_UwzgledniajCzescioweOtwory
                ext.OptimisePlanes = True
                ext.Step = 0#
                sf.AutoExtract2 ext
                If Err.Number = 0 Then extractionSukces = True
            End If

            ' Proba 2: Standardowe API SolidExtraction z dowolna orientacja (FeatureAnyOrientation = 2)
            If Not extractionSukces Then
                Err.Clear
                Set ext = sf.SolidExtraction
                If Not (ext Is Nothing) Then
                    ext.Orientation = FEAT_ORIENT_ANY ' 2: FeatureAnyOrientation
                    ext.Source = FEAT_ALL_BODIES_EXTRACTED
                    ext.FindContours = True
                    ext.FindDrillHoles = True
                    ext.MaxDrill = 150#
                    ext.IncludePartialHoles = g_UwzgledniajCzescioweOtwory
                    ext.OptimisePlanes = True
                    ext.Step = 0#
                    sf.AutoExtract ext
                End If
            End If
            On Error GoTo BladPrzetwarzania
        End If
    End If

    ' 5. Rozpoznanie gniazd Lamello P14 oraz klasyfikacja otworow na 6 stron (Faces 1..6)
    etapOpis = "Filtrowanie gniazd Lamello P14 i klasyfikacja otworow na 6 stron"
    Dim linieMaszynXml As New Collection
    Dim linieLamelloXml As New Collection

    ' Zbieramy wszystkie sciezki z rysunku (zarowno plaszczyzna glowna, jak i plaszczyzny robocze WorkPlanes)
    Dim wszystkieSciezki As New Collection
    Dim p As Path

    For Each p In drw.Geometries
        DodajSciezkeUnikalna p, wszystkieSciezki
    Next p

    Dim wp As WorkPlane
    On Error Resume Next
    If Not (drw.WorkPlanes Is Nothing) Then
        For Each wp In drw.WorkPlanes
            If Not (wp Is Nothing) Then
                For Each p In wp.Geometries
                    DodajSciezkeUnikalna p, wszystkieSciezki
                Next p
            End If
        Next wp
    End If
    On Error GoTo BladPrzetwarzania

    ' 5a. PRZEJSCIE 1 (LAMELLO P14): Dedykowana ekstrakcja i filtrowanie gniazd Lamello P14
    '     (zarowno od gory Face 5 jak i w 4 brzegach Face 1..4 + otwor techniczny fi 6 pod imbus)
    Dim wykryteLamello() As DaneGniazdaLamelloP14
    Dim liczbaWykrytychLamello As Long
    liczbaWykrytychLamello = 0
    WyodrebnijGniazdaLamelloP14 sf, wszystkieSciezki, L, W, T, linieMaszynXml, linieLamelloXml, wykryteLamello, liczbaWykrytychLamello
    outLiczbaLamello = linieLamelloXml.Count

    ' 5b. PRZEJSCIE 2 (OTWORY WIERCONE): Klasyfikacja standardowych otworow wierconych z wyekstrahowanych sciezek
    '     (calkowicie odseparowana od gniazd Lamello P14)
    Dim idxSciezki As Long
    For idxSciezki = 1 To wszystkieSciezki.Count
        Set p = wszystkieSciezki(idxSciezki)
        If CzySciezkaToOtwor(p) Then
            Dim xmlOtworu As String
            Dim unikalnyKlucz As String

            If SprobujUtworzycWpisOtworu(sf, p, L, W, T, zamienGzymki, kompensujStozek, g_UwzgledniajCzescioweOtwory, wykryteLamello, liczbaWykrytychLamello, xmlOtworu, unikalnyKlucz) Then
                ' Bezpieczne dodawanie z ignorowaniem duplikatow
                On Error Resume Next
                linieMaszynXml.Add xmlOtworu, unikalnyKlucz
                On Error GoTo BladPrzetwarzania
            End If
        End If
    Next idxSciezki

    ' 5c. Ekstrakcja bezposrednia otworow z bryl Solid (direct B-Rep scan) jako zabezpieczenie
    '     przed pominieciem otworow bocznych w trudnych modelach STEP
    EkstrahujOtworyDirectZSolidFaces sf, L, W, T, zamienGzymki, kompensujStozek, g_UwzgledniajCzescioweOtwory, wykryteLamello, liczbaWykrytychLamello, linieMaszynXml

    outLiczbaOtworow = linieMaszynXml.Count

    Dim fso As Object
    Set fso = CreateObject("Scripting.FileSystemObject")

    ' Jesli z pliku STEP nie zostala wygenerowana zadna operacja dla wiertarki (0 otworow i 0 gniazd Lamello),
    ' nie tworzymy programu SCX dla takiej formatki (oraz usuwamy ewentualny stary plik SCX)
    If outLiczbaOtworow = 0 And outLiczbaLamello = 0 Then
        outPominietoBrakOperacji = True
        On Error Resume Next
        If fso.FileExists(sciezkaScx) Then
            fso.DeleteFile sciezkaScx, True
        End If
        On Error GoTo BladPrzetwarzania
        PrzetworzPlikStepDoScx = True
        Exit Function
    End If

    ' 6. Generowanie struktury XML w formacie SCX dla Nanxing
    etapOpis = "Generowanie pliku XML SCX"
    Dim folderDocelowyScx As String
    folderDocelowyScx = fso.GetParentFolderName(sciezkaScx)
    If folderDocelowyScx <> "" Then
        If Not fso.FolderExists(folderDocelowyScx) Then
            fso.CreateFolder folderDocelowyScx
        End If
    End If

    Dim panelId As String
    panelId = fso.GetBaseName(sciezkaStep)

    Dim zawartoscXml As String
    zawartoscXml = ZbudujXmlScx(panelId, L, W, T, linieMaszynXml, linieLamelloXml)

    ' 7. Zapis pliku UTF-8 na dysk
    etapOpis = "Zapis pliku SCX"
    ZapiszPlikUtf8 sciezkaScx, zawartoscXml

    PrzetworzPlikStepDoScx = True
    Exit Function

BladPrzetwarzania:
    g_OstatniBlad = etapOpis & " -> Blad nr " & Err.Number & ": " & Err.Description
    PrzetworzPlikStepDoScx = False
End Function

' ==============================================================================
' ODCZYTYWANIE GLOBALNYCH WSPOLRZEDNYCH I GABARYTOW OTWORU (3D)
' ==============================================================================
Private Function PobierzWspolrzedneOtworu( _
    ByVal p As Path, _
    ByRef outCx As Double, ByRef outCy As Double, ByRef outCz As Double, _
    ByRef outMinX As Double, ByRef outMaxX As Double, _
    ByRef outMinY As Double, ByRef outMaxY As Double, _
    ByRef outMinZ As Double, ByRef outMaxZ As Double) As Boolean

    PobierzWspolrzedneOtworu = False
    outCx = 0#: outCy = 0#: outCz = 0#
    outMinX = 0#: outMaxX = 0#
    outMinY = 0#: outMaxY = 0#
    outMinZ = 0#: outMaxZ = 0#
    If p Is Nothing Then Exit Function

    On Error Resume Next
    Dim first As Boolean
    first = True

    ' Najdokladniejsza metoda: odczyt rzeczywistych punktow elementow sciezki w ukladzie globalnym (G)
    Dim elem As Object
    For Each elem In p.Elements
        If first Then
            outMinX = CDbl(elem.StartXG): outMaxX = outMinX
            outMinY = CDbl(elem.StartYG): outMaxY = outMinY
            outMinZ = CDbl(elem.StartZG): outMaxZ = outMinZ
            first = False
        Else
            If CDbl(elem.StartXG) < outMinX Then outMinX = CDbl(elem.StartXG)
            If CDbl(elem.StartXG) > outMaxX Then outMaxX = CDbl(elem.StartXG)
            If CDbl(elem.StartYG) < outMinY Then outMinY = CDbl(elem.StartYG)
            If CDbl(elem.StartYG) > outMaxY Then outMaxY = CDbl(elem.StartYG)
            If CDbl(elem.StartZG) < outMinZ Then outMinZ = CDbl(elem.StartZG)
            If CDbl(elem.StartZG) > outMaxZ Then outMaxZ = CDbl(elem.StartZG)
        End If

        If CDbl(elem.EndXG) < outMinX Then outMinX = CDbl(elem.EndXG)
        If CDbl(elem.EndXG) > outMaxX Then outMaxX = CDbl(elem.EndXG)
        If CDbl(elem.EndYG) < outMinY Then outMinY = CDbl(elem.EndYG)
        If CDbl(elem.EndYG) > outMaxY Then outMaxY = CDbl(elem.EndYG)
        If CDbl(elem.EndZG) < outMinZ Then outMinZ = CDbl(elem.EndZG)
        If CDbl(elem.EndZG) > outMaxZ Then outMaxZ = CDbl(elem.EndZG)

        If elem.IsArc Then
            If CDbl(elem.CenterXG) < outMinX Then outMinX = CDbl(elem.CenterXG)
            If CDbl(elem.CenterXG) > outMaxX Then outMaxX = CDbl(elem.CenterXG)
            If CDbl(elem.CenterYG) < outMinY Then outMinY = CDbl(elem.CenterYG)
            If CDbl(elem.CenterYG) > outMaxY Then outMaxY = CDbl(elem.CenterYG)
            If CDbl(elem.CenterZG) < outMinZ Then outMinZ = CDbl(elem.CenterZG)
            If CDbl(elem.CenterZG) > outMaxZ Then outMaxZ = CDbl(elem.CenterZG)
        End If
    Next elem

    If Not first Then
        outCx = (outMinX + outMaxX) / 2#
        outCy = (outMinY + outMaxY) / 2#
        outCz = (outMinZ + outMaxZ) / 2#

        Dim cp As Object
        Set cp = p.GetCircleProperties
        If Not (cp Is Nothing) Then
            outCx = CDbl(cp.CenterXG)
            outCy = CDbl(cp.CenterYG)
            outCz = CDbl(cp.CenterZG)
        End If

        PobierzWspolrzedneOtworu = True
        Exit Function
    End If

    ' Fallback gdyby elements bylo puste
    Dim cpFallback As Object
    Set cpFallback = p.GetCircleProperties
    If Not (cpFallback Is Nothing) Then
        outCx = CDbl(cpFallback.CenterXG)
        outCy = CDbl(cpFallback.CenterYG)
        outCz = CDbl(cpFallback.CenterZG)
        Dim r As Double
        r = CDbl(cpFallback.Radius)
        outMinX = outCx - r: outMaxX = outCx + r
        outMinY = outCy - r: outMaxY = outCy + r
        outMinZ = outCz: outMaxZ = outCz
        PobierzWspolrzedneOtworu = True
    End If
    On Error GoTo 0
End Function

' ==============================================================================
' KLASYFIKACJA GEOMETRYCZNA OTWORU I GENEROWANIE LINII XML
' ==============================================================================
Private Function SprobujUtworzycWpisOtworu( _
    ByVal sf As Object, _
    ByVal p As Path, _
    ByVal L As Double, ByVal W As Double, ByVal T As Double, _
    ByVal zamienGzymki As Boolean, _
    ByVal kompensujStozek As Boolean, _
    ByVal uwzgledniajCzesciowe As Boolean, _
    ByRef wykryteLamello() As DaneGniazdaLamelloP14, _
    ByVal liczbaWykrytychLamello As Long, _
    ByRef outXmlLinia As String, _
    ByRef outKlucz As String) As Boolean

    SprobujUtworzycWpisOtworu = False
    outXmlLinia = ""
    outKlucz = ""
    If p Is Nothing Then Exit Function

    Dim dia As Double
    dia = PobierzSredniceOtworu(p)
    If dia <= 0.1 Then Exit Function

    ' Odfiltrowanie powierzchni tarczy Lamello P14 (~100.4 mm) oraz duzych kieszeni niebedacych wierceniami
    If dia >= (LAMELLO_STEP_RAD_MIN * 2#) And dia <= (LAMELLO_STEP_RAD_MAX * 2#) Then Exit Function
    If dia > 45# Then Exit Function

    Dim diaOryg As Double
    diaOryg = dia

    ' Opcjonalna zamiana gzymki fi 11 na poglebienie proste fi 12
    If zamienGzymki Then
        If Abs(dia - 11#) < 0.6 Or Abs(dia - 10.8) < 0.4 Then
            dia = 12#
        End If
    End If

    Dim dep As Double
    dep = PobierzGlebokoscOtworu(p)

    Dim minX As Double, maxX As Double
    Dim minY As Double, maxY As Double
    Dim minZ As Double, maxZ As Double
    Dim cx As Double, cy As Double, cz As Double
    If Not PobierzWspolrzedneOtworu(p, cx, cy, cz, minX, maxX, minY, maxY, minZ, maxZ) Then
        Exit Function
    End If

    Dim extX As Double, extY As Double, extZ As Double
    extX = maxX - minX
    extY = maxY - minY
    extZ = maxZ - minZ

    ' Odfiltrowanie prostokatnych i lukowych konturow gniazda Lamello P14 (~69.56 x 10 x 14 mm)
    If CzyWymiaryProstokataToLamelloP14(extX, extY, extZ) Then Exit Function
    If extX > 45# Or extY > 45# Or extZ > 45# Then Exit Function

    ' Jesli czesciowe otwory sa wylaczone (domyslnie False), odrzucamy otwarte luki i niepelne wycinki (np. promienie R w naroznikach)
    If Not uwzgledniajCzesciowe Then
        On Error Resume Next
        If Not p.Closed And Not p.IsDrilling Then
            On Error GoTo 0
            Exit Function
        End If
        On Error GoTo 0
    End If

    ' Odczyt globalnego wektora narzedzia z elementu (Tool Vector)
    Dim tvX As Double, tvY As Double, tvZ As Double
    tvX = 0#: tvY = 0#: tvZ = 0#
    On Error Resume Next
    If Not (p.GetFirstElem Is Nothing) Then
        tvX = Abs(p.GetFirstElem.StartToolVectorXG)
        tvY = Abs(p.GetFirstElem.StartToolVectorYG)
        tvZ = Abs(p.GetFirstElem.StartToolVectorZG)
    End If
    On Error GoTo 0

    ' Nazwa plaszczyzny roboczej
    Dim wpNazwa As String
    wpNazwa = ""
    On Error Resume Next
    If Not (p.GetWorkPlane Is Nothing) Then
        wpNazwa = UCase$(p.GetWorkPlane.Name)
    End If
    On Error GoTo 0

    Dim isWP_TopBot As Boolean, isWP_FrontBack As Boolean, isWP_LeftRight As Boolean
    isWP_TopBot = (InStr(1, wpNazwa, "TOP", vbTextCompare) > 0 Or InStr(1, wpNazwa, "GÓR", vbTextCompare) > 0 Or _
                   InStr(1, wpNazwa, "GOR", vbTextCompare) > 0 Or InStr(1, wpNazwa, "BOTTOM", vbTextCompare) > 0 Or _
                   InStr(1, wpNazwa, "DÓŁ", vbTextCompare) > 0 Or InStr(1, wpNazwa, "DOL", vbTextCompare) > 0)

    isWP_FrontBack = (InStr(1, wpNazwa, "BACK", vbTextCompare) > 0 Or InStr(1, wpNazwa, "TYŁ", vbTextCompare) > 0 Or _
                      InStr(1, wpNazwa, "TYL", vbTextCompare) > 0 Or InStr(1, wpNazwa, "FRONT", vbTextCompare) > 0 Or _
                      InStr(1, wpNazwa, "PRZÓD", vbTextCompare) > 0 Or InStr(1, wpNazwa, "PRZOD", vbTextCompare) > 0)

    isWP_LeftRight = (InStr(1, wpNazwa, "RIGHT", vbTextCompare) > 0 Or InStr(1, wpNazwa, "PRAW", vbTextCompare) > 0 Or _
                      InStr(1, wpNazwa, "LEFT", vbTextCompare) > 0 Or InStr(1, wpNazwa, "LEW", vbTextCompare) > 0)

    Dim tTyp As Long, tFace As Long
    Dim pX As Double, pY As Double, pZ As Double

    ' --- 1. SPRAWDZENIE POZIOME W OSI Y (XZ -> TYL Face 1 / PRZOD Face 2) ---
    If isWP_FrontBack Or tvY > 0.7 Or (extY < 0.25 And extX > 0.5 And extZ > 0.5) Or (extY < extX * 0.25 And extY < extZ * 0.25) Then
        tTyp = 1
        pX = cx
        pZ = cz  ' Rzeczywista wysokosc Z od spodu formatki Z=0
        If InStr(1, wpNazwa, "BACK", vbTextCompare) > 0 Or InStr(1, wpNazwa, "TYŁ", vbTextCompare) > 0 Or InStr(1, wpNazwa, "TYL", vbTextCompare) > 0 Or cy >= (W / 2#) Then
            tFace = 1 ' TYL (Y = W)
            pY = W
        Else
            tFace = 2 ' PRZOD (Y = 0)
            pY = 0#
        End If

    ' --- 2. SPRAWDZENIE POZIOME W OSI X (YZ -> PRAWO Face 3 / LEWO Face 4) ---
    ElseIf isWP_LeftRight Or tvX > 0.7 Or (extX < 0.25 And extY > 0.5 And extZ > 0.5) Or (extX < extY * 0.25 And extX < extZ * 0.25) Then
        tTyp = 1
        pY = cy
        pZ = cz  ' Rzeczywista wysokosc Z od spodu formatki Z=0
        If InStr(1, wpNazwa, "RIGHT", vbTextCompare) > 0 Or InStr(1, wpNazwa, "PRAW", vbTextCompare) > 0 Or cx >= (L / 2#) Then
            tFace = 3 ' PRAWO (X = L)
            pX = L
        Else
            tFace = 4 ' LEWO (X = 0)
            pX = 0#
        End If

    ' --- 3. SPRAWDZENIE PIONOWE W OSI Z (XY -> GORA Face 5 / DOL Face 6) ---
    ElseIf isWP_TopBot Or tvZ > 0.7 Or (extZ < 0.25 And extX > 0.5 And extY > 0.5) Or (extZ < extX * 0.25 And extZ < extY * 0.25) Then
        tTyp = 2
        pX = cx
        pY = cy
        pZ = 0#
        tFace = 5 ' Domyslnie Face 5 (GORA - zarowno otwory od gory jak i przelotowe)

        ' Sprawdzenie czy nie jest to slepy otwor wchodzacy wylacznie od spodu
        Dim vTop As Variant, vBot As Variant
        vTop = p.Attribute(ATTR_GEO_Z_LEVEL_TOP)
        vBot = p.Attribute(ATTR_GEO_Z_LEVEL_BOTTOM)
        If Not IsEmpty(vTop) And Not IsEmpty(vBot) Then
            Dim zWyzszy As Double, zNizszy As Double
            zWyzszy = CDbl(vTop): zNizszy = CDbl(vBot)
            If zNizszy > zWyzszy Then
                Dim tmpSwap As Double: tmpSwap = zWyzszy: zWyzszy = zNizszy: zNizszy = tmpSwap
            End If
            ' Slepy otwor od spodu (nie siega do gory formatki)
            If zNizszy < 1# And zWyzszy < (T - 2#) And (InStr(1, wpNazwa, "BOTTOM", vbTextCompare) > 0 Or InStr(1, wpNazwa, "DÓŁ", vbTextCompare) > 0 Or InStr(1, wpNazwa, "DOL", vbTextCompare) > 0) Then
                tFace = 6
            End If
        End If

    ' --- 4. FALLBACK: OSTATNIA DESKA RATUNKU (DOMYŚLNIE PION FACE 5) ---
    Else
        tTyp = 2
        pX = cx
        pY = cy
        pZ = 0#
        tFace = 5
    End If

    ' Ograniczenie wspolrzednych do granic formatki
    If pX < 0# Then pX = 0#
    If pX > L Then pX = L
    If pY < 0# Then pY = 0#
    If pY > W Then pY = W
    If pZ < 0# Then pZ = 0#
    If pZ > T Then pZ = T

    ' Jesli jest to pionowy otwor techniczny fi 6 pod klucz imbus dla wykrytego gniazda Lamello P14 w brzegu,
    ' pomijamy go tutaj, poniewaz zostal juz precyzyjnie dodany na Face 5 z docelowa glebokoscia do srodka gniazda (T - Z)
    If tTyp = 2 And CzyOtworToTechnicznyImbusLamelloP14(pX, pY, dia, L, W, wykryteLamello, liczbaWykrytychLamello) Then
        Exit Function
    End If

    ' Weryfikacja w bryle B-Rep 3D:
    ' 1) Odrzucenie niepelnych otworow / promieni w naroznikach podciec (jesli uwzgledniajCzesciowe = False)
    ' 2) Wyznaczenie rzeczywistej glebokosci od powierzchni wejscia do dna otworu (oraz +2 mm dla otworow poziomych bez dna)
    If Not WeryfikujOtworZBRep(sf, tTyp, tFace, pX, pY, pZ, diaOryg, L, W, T, uwzgledniajCzesciowe, dep) Then
        Exit Function
    End If

    ' Weryfikacja i zabezpieczenie glebokosci (ochrona przed artefaktami atrybutow przed obrotem)
    If tTyp = 2 Then
        If dep > T Or dep <= 0.05 Then
            dep = T
        End If
    ElseIf tTyp = 1 Then
        If tFace = 1 Or tFace = 2 Then
            If dep > (W - 0.5) Or dep <= 0.05 Then dep = 25#
        ElseIf tFace = 3 Or tFace = 4 Then
            If dep > (L - 0.5) Or dep <= 0.05 Then dep = 25#
        End If
    End If

    ' Kompensacja stozka wiertla 118 stopni (standard Nanxing)
    ' Dodawana do nieprzelotowych otworow wierconych wiertlem kretym (D <= 12.5 mm)
    If kompensujStozek And dia <= 12.5 Then
        Dim czyNieprzelotowy As Boolean
        czyNieprzelotowy = False
        If tTyp = 2 Then
            ' Pionowy: nieprzelotowy jesli glebokosc mniejsza niz grubosc formatki T
            If dep < (T - 0.2) Then czyNieprzelotowy = True
        ElseIf tTyp = 1 Then
            ' Poziomy: na krawedziach bokow wiercenia sa slepe (zazwyczaj 15-45 mm)
            ' Sprawdzamy czy nie przebija na wylot calej formatki
            If (tFace = 1 Or tFace = 2) And dep < (W - 0.5) Then
                czyNieprzelotowy = True
            ElseIf (tFace = 3 Or tFace = 4) And dep < (L - 0.5) Then
                czyNieprzelotowy = True
            End If
        End If

        If czyNieprzelotowy Then
            Dim stozekWiertla As Double
            ' h = (D / 2) / tan(59 deg), gdzie 59 deg = 1.02974425867665 rad
            stozekWiertla = (dia / 2#) / Tan(1.02974425867665)
            dep = dep + stozekWiertla
        End If
    End If

    ' Bezwzgledny limit maksymalnej glebokosci wiercenia bocznego (sciany 1..4) do 50 mm (dlugosc robocza wiertla)
    If tTyp = 1 Then
        If dep > MAX_GLEBOKOSC_WIERCENIA_BOCZNEGO Then
            dep = MAX_GLEBOKOSC_WIERCENIA_BOCZNEGO
        End If
    End If

    ' Unikalny klucz do wykrywania duplikatow okregow w STEP
    If tTyp = 2 Then
        outKlucz = "F" & tFace & "_X" & Format$(pX, "0.0") & "_Y" & Format$(pY, "0.0") & _
                   "_D" & Format$(dia, "0.0")
    Else
        outKlucz = "F" & tFace & "_X" & Format$(pX, "0.0") & "_Y" & Format$(pY, "0.0") & _
                   "_Z" & Format$(pZ, "0.0") & "_D" & Format$(dia, "0.0")
    End If

    ' Generowanie linii XML
    If tTyp = 1 Then
        outXmlLinia = "          <Machining Type=""1"" IsGenCode=""2"" Face=""" & tFace & """" & _
                      " X=""" & FormatLiczbaXml(pX) & """" & _
                      " Y=""" & FormatLiczbaXml(pY) & """" & _
                      " Z=""" & FormatLiczbaXml(pZ) & """" & _
                      " Diameter=""" & FormatLiczbaXml(dia) & """" & _
                      " Depth=""" & FormatLiczbaXml(dep) & """ />"
    Else
        outXmlLinia = "          <Machining Type=""2"" IsGenCode=""2"" Face=""" & tFace & """" & _
                      " X=""" & FormatLiczbaXml(pX) & """" & _
                      " Y=""" & FormatLiczbaXml(pY) & """" & _
                      " Diameter=""" & FormatLiczbaXml(dia) & """" & _
                      " Depth=""" & FormatLiczbaXml(dep) & """ />"
    End If

    SprobujUtworzycWpisOtworu = True
End Function

' ==============================================================================
' BUDOWANIE PLIKU XML SCX DLA WIERTARKI NANXING
' ==============================================================================
Private Function ZbudujXmlScx( _
    ByVal panelId As String, _
    ByVal L As Double, ByVal W As Double, ByVal T As Double, _
    ByVal linieOtworow As Collection, _
    Optional ByVal linieLamello As Collection = Nothing) As String

    Dim sb As String
    sb = "<?xml version=""1.0"" encoding=""utf-8""?>" & vbCrLf
    sb = sb & "<Root Cad=""BuiltInCad"" version=""2.0"">" & vbCrLf
    sb = sb & "  <Project>" & vbCrLf
    sb = sb & "    <Panels>" & vbCrLf
    sb = sb & "      <Panel IsProduce=""true"" ID=""" & panelId & """ Name="""" Length=""" & _
         FormatLiczbaXml(L) & """ Width=""" & FormatLiczbaXml(W) & """ Thickness=""" & _
         FormatLiczbaXml(T) & """ MachiningPoint=""1"">" & vbCrLf

    ' 1. Outline (obrys prostokatny formatki)
    sb = sb & "        <Outline>" & vbCrLf
    sb = sb & "          <Point X=""0"" Y=""0"" />" & vbCrLf
    sb = sb & "          <Point X=""" & FormatLiczbaXml(L) & """ Y=""0"" />" & vbCrLf
    sb = sb & "          <Point X=""" & FormatLiczbaXml(L) & """ Y=""" & FormatLiczbaXml(W) & """ />" & vbCrLf
    sb = sb & "          <Point X=""0"" Y=""" & FormatLiczbaXml(W) & """ />" & vbCrLf
    sb = sb & "        </Outline>" & vbCrLf
    sb = sb & "        <Innerlines />" & vbCrLf

    ' 2. Machines (operacje wiercenia oraz gniazda Lamello P14 wyekstrahowane z bryly)
    sb = sb & "        <Machines>" & vbCrLf
    Dim linia As Variant
    If Not (linieOtworow Is Nothing) Then
        For Each linia In linieOtworow
            sb = sb & CStr(linia) & vbCrLf
        Next linia
    End If
    If Not (linieLamello Is Nothing) Then
        For Each linia In linieLamello
            sb = sb & CStr(linia) & vbCrLf
        Next linia
    End If
    sb = sb & "        </Machines>" & vbCrLf

    ' 3. EdgeGroup (standardowa 4-krawedziowa grupa Nanxing)
    sb = sb & "        <EdgeGroup X1=""0"" Y1=""0"">" & vbCrLf
    sb = sb & "          <Edge Face=""2"" Thickness=""0"" Pre_Milling=""0"" X=""0"" Y=""0"" CentralAngle=""0"" />" & vbCrLf
    sb = sb & "          <Edge Face=""1"" Thickness=""0"" Pre_Milling=""0"" X=""0"" Y=""0"" CentralAngle=""0"" />" & vbCrLf
    sb = sb & "          <Edge Face=""4"" Thickness=""0"" Pre_Milling=""0"" X=""0"" Y=""0"" CentralAngle=""0"" />" & vbCrLf
    sb = sb & "          <Edge Face=""3"" Thickness=""0"" Pre_Milling=""0"" X=""0"" Y=""0"" CentralAngle=""0"" />" & vbCrLf
    sb = sb & "        </EdgeGroup>" & vbCrLf

    sb = sb & "      </Panel>" & vbCrLf
    sb = sb & "    </Panels>" & vbCrLf
    sb = sb & "  </Project>" & vbCrLf
    sb = sb & "</Root>" & vbCrLf

    ZbudujXmlScx = sb
End Function

' ==============================================================================
' DEDYKOWANA EKSTRAKCJA I FILTROWANIE GNIAZD LAMELLO P14 Z MODELU STEP (3D B-REP)
' Obsluguje:
'   1. Gniazda w powierzchni gornej (Face 5, Z=T) oraz dolnej (Face 6, Z=0):
'      <Machining Type="1002" IsGenCode="2" Face="5" Direction="X/Y" X="..." Y="..." Z="18" Length="66" Width="9.8" MaxDepth="14.1" />
'   2. Gniazda w 4 scianach bocznych (brzegu formatki: Face 1, 2, 3, 4) wykonywane tarcza:
'      <Machining Type="1002" IsGenCode="2" Face="1..4" Direction="X/Y" X="..." Y="..." Z="..." Length="66" Width="10.15" MaxDepth="14.1" />
'      + automatycznie dopisywany otwor techniczny fi 6 pod klucz imbus na Face 5
'        (idealnie na srodku gniazda, 7.5 mm od brzegu gniazda formatki,
'         glebokosc do srodka wysokosci gniazda: Depth = T - Z, bez kompensacji stozka wiertla).
' ==============================================================================
Private Sub WyodrebnijGniazdaLamelloP14( _
    ByVal sf As Object, _
    ByVal wszystkieSciezki As Collection, _
    ByVal L As Double, ByVal W As Double, ByVal T As Double, _
    ByRef linieMaszynXml As Collection, _
    ByRef linieLamelloXml As Collection, _
    ByRef wykryte() As DaneGniazdaLamelloP14, _
    ByRef liczbaWykrytych As Long)

    On Error Resume Next
    liczbaWykrytych = 0
    If sf Is Nothing Then Exit Sub
    If sf.Bodies.Count = 0 Then Exit Sub

    ' KROK 1: Skanowanie cylindrycznych powierzchni dna gniazda Lamello P14 (R ~ 50.2 mm, D ~ 100.4 mm)
    If Not (sf Is Nothing) Then
        If sf.Bodies.Count > 0 Then
            Dim b As Object
            Dim f As Object
            For Each b In sf.Bodies
                For Each f In b.Faces
                    Dim g As DaneGniazdaLamelloP14
                    If AnalizujScianeCylindrycznaLamelloP14(f, L, W, T, g) Then
                        DodajUnikalneGniazdoLamelloP14 wykryte, liczbaWykrytych, g
                    End If
                Next f
            Next b

            ' KROK 2: Skanowanie petli wewnetrznych na 6 scianach plaskich bryly (~69.56 x 10.0 mm)
            SkanujPetlePlaskieLamelloP14 sf, L, W, T, wykryte, liczbaWykrytych
        End If
    End If

    ' KROK 3: Skanowanie wyekstrahowanych sciezek 2D/3D (w tym lukow tarczy D ~ 100.4 mm z AutoExtract2)
    If Not (wszystkieSciezki Is Nothing) Then
        SkanujSciezkiLamelloP14 wszystkieSciezki, L, W, T, wykryte, liczbaWykrytych
    End If

    ' KROK 4: Generowanie wpisow XML do sekcji <Machines>
    Dim i As Long
    For i = 1 To liczbaWykrytych
        DodajWpisGniazdaLamelloP14 wykryte(i), L, W, T, linieMaszynXml, linieLamelloXml
    Next i
    On Error GoTo 0
End Sub

' ==============================================================================
' ANALIZA POJEDYNCZEJ SCIANY CYLINDRYCZNEJ B-REP POD KATEM GNIAZDA LAMELLO P14
' ==============================================================================
Private Function AnalizujScianeCylindrycznaLamelloP14( _
    ByVal f As Object, _
    ByVal L As Double, ByVal W As Double, ByVal T As Double, _
    ByRef outGniazdo As DaneGniazdaLamelloP14) As Boolean

    AnalizujScianeCylindrycznaLamelloP14 = False
    If f Is Nothing Then Exit Function
    On Error Resume Next

    Dim fType As Long
    fType = -1
    fType = CLng(f.Type)
    If fType <> 2 Then Exit Function ' 2 = FeatureCylindricalFace

    Dim c As Object
    Set c = f.Conic
    If c Is Nothing Then Exit Function
    If Not CBool(c.Hollow) Then Exit Function

    Dim rad As Double
    rad = 0#
    Err.Clear
    rad = CDbl(c.Radius.Value)
    If Err.Number <> 0 Or rad <= 0# Then
        Err.Clear
        rad = CDbl(c.Radius)
    End If

    ' Odczyt osi i wektora osi cylindra (c.Axis.P oraz c.Axis.V)
    Dim axX As Double, axY As Double, axZ As Double
    axX = 0#: axY = 0#: axZ = 0#
    Dim apx As Double, apy As Double, apz As Double
    Dim hasAxisPt As Boolean: hasAxisPt = False

    Dim axObj As Object
    Set axObj = Nothing
    Set axObj = c.Axis
    If Not (axObj Is Nothing) Then
        Dim axPt As Object
        Set axPt = Nothing
        Set axPt = axObj.P
        If Not (axPt Is Nothing) Then
            If PobierzPunkt3D(axPt, apx, apy, apz) Then
                hasAxisPt = True
            End If
        End If

        Err.Clear
        If Not (axObj.V Is Nothing) Then
            axX = Abs(CDbl(axObj.V.dx))
            axY = Abs(CDbl(axObj.V.dy))
            axZ = Abs(CDbl(axObj.V.dz))
            If Err.Number <> 0 Then
                Err.Clear
                axX = Abs(CDbl(axObj.V.dxDouble))
                axY = Abs(CDbl(axObj.V.dyDouble))
                axZ = Abs(CDbl(axObj.V.dzDouble))
            End If
        End If
        If (axX + axY + axZ) < 0.0001 Then
            Err.Clear
            axX = Abs(CDbl(axObj.dx))
            axY = Abs(CDbl(axObj.dy))
            axZ = Abs(CDbl(axObj.dz))
        End If
    End If

    ' Pobranie srodkow krawedzi kolowych (lukow R ~ 50.2 mm)
    Dim minCenX As Double, maxCenX As Double
    Dim minCenY As Double, maxCenY As Double
    Dim minCenZ As Double, maxCenZ As Double
    Dim hasCen As Boolean
    hasCen = False

    Dim edg As Object
    For Each edg In f.Edges
        If CLng(edg.Type) = 2 Then ' FeatureCircularEdge = 2
            If rad <= 0# Then
                Err.Clear
                rad = CDbl(edg.Radius.Value)
                If Err.Number <> 0 Or rad <= 0# Then
                    Err.Clear
                    rad = CDbl(edg.Radius)
                End If
            End If
            Dim cpt As Object
            Set cpt = Nothing
            Set cpt = edg.Center
            If Not (cpt Is Nothing) Then
                Dim cx As Double, cy As Double, cz As Double
                If PobierzPunkt3D(cpt, cx, cy, cz) Then
                    If Not hasCen Then
                        minCenX = cx: maxCenX = cx
                        minCenY = cy: maxCenY = cy
                        minCenZ = cz: maxCenZ = cz
                        hasCen = True
                    Else
                        If cx < minCenX Then minCenX = cx
                        If cx > maxCenX Then maxCenX = cx
                        If cy < minCenY Then minCenY = cy
                        If cy > maxCenY Then maxCenY = cy
                        If cz < minCenZ Then minCenZ = cz
                        If cz > maxCenZ Then maxCenZ = cz
                    End If
                End If
            End If
        End If
    Next edg

    ' Sprawdzenie czy promien miesci sie w tolerancji tarczy Lamello P14 (~50.2 mm)
    If rad < LAMELLO_STEP_RAD_MIN Or rad > LAMELLO_STEP_RAD_MAX Then
        On Error GoTo 0
        Exit Function
    End If

    ' Pobranie zakresu wierzcholkow / koncow krawedzi sciany
    Dim vMinX As Double, vMaxX As Double
    Dim vMinY As Double, vMaxY As Double
    Dim vMinZ As Double, vMaxZ As Double
    Dim hasVert As Boolean
    hasVert = PobierzZakresWierzcholkowSciany(f, vMinX, vMaxX, vMinY, vMaxY, vMinZ, vMaxZ)

    ' Jesli brak srodkow z krawedzi, uzywamy punktu osi cylindra (c.Axis.P)
    If Not hasCen And hasAxisPt Then
        minCenX = apx: maxCenX = apx
        minCenY = apy: maxCenY = apy
        minCenZ = apz: maxCenZ = apz
        hasCen = True
    End If

    If Not hasCen And Not hasVert Then
        On Error GoTo 0
        Exit Function
    End If

    Dim midCenX As Double, midCenY As Double, midCenZ As Double
    Dim spanCenX As Double, spanCenY As Double, spanCenZ As Double
    If hasCen Then
        midCenX = (minCenX + maxCenX) / 2#
        midCenY = (minCenY + maxCenY) / 2#
        midCenZ = (minCenZ + maxCenZ) / 2#
        spanCenX = maxCenX - minCenX
        spanCenY = maxCenY - minCenY
        spanCenZ = maxCenZ - minCenZ
    End If

    Dim vMidX As Double, vMidY As Double, vMidZ As Double
    Dim vSpanX As Double, vSpanY As Double, vSpanZ As Double
    If hasVert Then
        vMidX = (vMinX + vMaxX) / 2#
        vMidY = (vMinY + vMaxY) / 2#
        vMidZ = (vMinZ + vMaxZ) / 2#
        vSpanX = vMaxX - vMinX
        vSpanY = vMaxY - vMinY
        vSpanZ = vMaxZ - vMinZ
    End If

    Dim wykrFace As Long: wykrFace = 0
    Dim wykrDir As String: wykrDir = ""
    Dim wykrX As Double: wykrX = 0#
    Dim wykrY As Double: wykrY = 0#
    Dim wykrZ As Double: wykrZ = 0#
    Dim wykrDep As Double: wykrDep = 0#
    Dim widX As Double, widY As Double, widZ As Double

    If spanCenX > 1# Then widX = spanCenX Else widX = vSpanX
    If spanCenY > 1# Then widY = spanCenY Else widY = vSpanY
    If spanCenZ > 1# Then widZ = spanCenZ Else widZ = vSpanZ

    ' Uwaga: W modelach SolidCraft powierzchnia cylindryczna gniazda moze byc podzielona szwem
    ' na dwie czesci (np. 63.66 mm + 5.89 mm), dlatego gdy znamy srodki okregow (hasCen = True),
    ' uzywamy bezposrednio midCenX / midCenY / midCenZ jako geometrycznego srodka tarczy!

    ' --- 1. POWIERZCHNIA GORNA (Face 5, Z = T) ---
    If (hasCen And midCenZ > (T + 10#)) Or (Not hasCen And hasVert And vSpanZ <= 1.2 And Abs(vMaxZ - T) <= 1.2) Then
        If hasCen And midCenZ > (T + 10#) Then
            wykrDep = rad - (midCenZ - T)
        Else
            wykrDep = 14#
        End If
        If wykrDep >= LAMELLO_STEP_DEP_MIN And wykrDep <= LAMELLO_STEP_DEP_MAX Then
            If widX >= LAMELLO_STEP_WID_MIN And widX <= LAMELLO_STEP_WID_MAX Then
                wykrFace = 5
                wykrDir = "Y"
                If spanCenX > 1# Then wykrX = midCenX Else wykrX = vMidX
                If hasCen Then wykrY = midCenY Else wykrY = vMidY
                wykrZ = T
            ElseIf widY >= LAMELLO_STEP_WID_MIN And widY <= LAMELLO_STEP_WID_MAX Then
                wykrFace = 5
                wykrDir = "X"
                If hasCen Then wykrX = midCenX Else wykrX = vMidX
                If spanCenY > 1# Then wykrY = midCenY Else wykrY = vMidY
                wykrZ = T
            ElseIf axX > 0.7 And hasVert And vSpanX >= 2# Then
                wykrFace = 5: wykrDir = "Y": wykrX = vMidX: wykrY = midCenY: wykrZ = T
            ElseIf axY > 0.7 And hasVert And vSpanY >= 2# Then
                wykrFace = 5: wykrDir = "X": wykrX = midCenX: wykrY = vMidY: wykrZ = T
            End If
        End If

    ' --- 2. POWIERZCHNIA DOLNA (Z = 0 -> w maszynie Nanxing tylko gorne wrzeciono Face 5 pobiera frez) ---
    ElseIf (hasCen And midCenZ < -10#) Or (Not hasCen And hasVert And vSpanZ <= 1.2 And Abs(vMinZ) <= 1.2) Then
        If hasCen And midCenZ < -10# Then
            wykrDep = rad + midCenZ
        Else
            wykrDep = 14#
        End If
        If wykrDep >= LAMELLO_STEP_DEP_MIN And wykrDep <= LAMELLO_STEP_DEP_MAX Then
            If widX >= LAMELLO_STEP_WID_MIN And widX <= LAMELLO_STEP_WID_MAX Then
                wykrFace = 5
                wykrDir = "Y"
                If spanCenX > 1# Then wykrX = midCenX Else wykrX = vMidX
                If hasCen Then wykrY = midCenY Else wykrY = vMidY
                wykrZ = T
            ElseIf widY >= LAMELLO_STEP_WID_MIN And widY <= LAMELLO_STEP_WID_MAX Then
                wykrFace = 5
                wykrDir = "X"
                If hasCen Then wykrX = midCenX Else wykrX = vMidX
                If spanCenY > 1# Then wykrY = midCenY Else wykrY = vMidY
                wykrZ = T
            ElseIf axX > 0.7 And hasVert And vSpanX >= 2# Then
                wykrFace = 5: wykrDir = "Y": wykrX = vMidX: wykrY = midCenY: wykrZ = T
            ElseIf axY > 0.7 And hasVert And vSpanY >= 2# Then
                wykrFace = 5: wykrDir = "X": wykrX = midCenX: wykrY = vMidY: wykrZ = T
            End If
        End If

    ' --- 3. BRZEG LEWY (Face 4, X = 0) ---
    ElseIf (hasCen And midCenX < -10#) Or (Not hasCen And hasVert And vSpanX <= 1.2 And Abs(vMinX) <= 1.2) Then
        If hasCen And midCenX < -10# Then
            wykrDep = rad + midCenX
        Else
            wykrDep = 14#
        End If
        If wykrDep >= LAMELLO_STEP_DEP_MIN And wykrDep <= LAMELLO_STEP_DEP_MAX Then
            If (widZ >= LAMELLO_STEP_WID_MIN And widZ <= LAMELLO_STEP_WID_MAX) Or axZ > 0.7 Then
                wykrFace = 4
                wykrDir = "Y"
                wykrX = 0#
                If hasCen Then wykrY = midCenY Else wykrY = vMidY
                If spanCenZ > 1# Then
                    wykrZ = midCenZ
                ElseIf hasVert And vSpanZ >= 2# Then
                    wykrZ = vMidZ
                Else
                    wykrZ = T / 2#
                End If
            End If
        End If

    ' --- 4. BRZEG PRAWY (Face 3, X = L) ---
    ElseIf (hasCen And midCenX > (L + 10#)) Or (Not hasCen And hasVert And vSpanX <= 1.2 And Abs(vMaxX - L) <= 1.2) Then
        If hasCen And midCenX > (L + 10#) Then
            wykrDep = rad - (midCenX - L)
        Else
            wykrDep = 14#
        End If
        If wykrDep >= LAMELLO_STEP_DEP_MIN And wykrDep <= LAMELLO_STEP_DEP_MAX Then
            If (widZ >= LAMELLO_STEP_WID_MIN And widZ <= LAMELLO_STEP_WID_MAX) Or axZ > 0.7 Then
                wykrFace = 3
                wykrDir = "Y"
                wykrX = L
                If hasCen Then wykrY = midCenY Else wykrY = vMidY
                If spanCenZ > 1# Then
                    wykrZ = midCenZ
                ElseIf hasVert And vSpanZ >= 2# Then
                    wykrZ = vMidZ
                Else
                    wykrZ = T / 2#
                End If
            End If
        End If

    ' --- 5. BRZEG PRZEDNI (Face 2, Y = 0) ---
    ElseIf (hasCen And midCenY < -10#) Or (Not hasCen And hasVert And vSpanY <= 1.2 And Abs(vMinY) <= 1.2) Then
        If hasCen And midCenY < -10# Then
            wykrDep = rad + midCenY
        Else
            wykrDep = 14#
        End If
        If wykrDep >= LAMELLO_STEP_DEP_MIN And wykrDep <= LAMELLO_STEP_DEP_MAX Then
            If (widZ >= LAMELLO_STEP_WID_MIN And widZ <= LAMELLO_STEP_WID_MAX) Or axZ > 0.7 Then
                wykrFace = 2
                wykrDir = "X"
                If hasCen Then wykrX = midCenX Else wykrX = vMidX
                wykrY = 0#
                If spanCenZ > 1# Then
                    wykrZ = midCenZ
                ElseIf hasVert And vSpanZ >= 2# Then
                    wykrZ = vMidZ
                Else
                    wykrZ = T / 2#
                End If
            End If
        End If

    ' --- 6. BRZEG TYLNY (Face 1, Y = W) ---
    ElseIf (hasCen And midCenY > (W + 10#)) Or (Not hasCen And hasVert And vSpanY <= 1.2 And Abs(vMaxY - W) <= 1.2) Then
        If hasCen And midCenY > (W + 10#) Then
            wykrDep = rad - (midCenY - W)
        Else
            wykrDep = 14#
        End If
        If wykrDep >= LAMELLO_STEP_DEP_MIN And wykrDep <= LAMELLO_STEP_DEP_MAX Then
            If (widZ >= LAMELLO_STEP_WID_MIN And widZ <= LAMELLO_STEP_WID_MAX) Or axZ > 0.7 Then
                wykrFace = 1
                wykrDir = "X"
                If hasCen Then wykrX = midCenX Else wykrX = vMidX
                wykrY = W
                If spanCenZ > 1# Then
                    wykrZ = midCenZ
                ElseIf hasVert And vSpanZ >= 2# Then
                    wykrZ = vMidZ
                Else
                    wykrZ = T / 2#
                End If
            End If
        End If
    End If

    If wykrFace >= 1 And wykrFace <= 6 And wykrDir <> "" Then
        If wykrX < 0# Then wykrX = 0#
        If wykrX > L Then wykrX = L
        If wykrY < 0# Then wykrY = 0#
        If wykrY > W Then wykrY = W
        If wykrZ < 0# Then wykrZ = 0#
        If wykrZ > T Then wykrZ = T

        outGniazdo.face = wykrFace
        outGniazdo.kierunek = wykrDir
        outGniazdo.x = wykrX
        outGniazdo.y = wykrY
        outGniazdo.z = wykrZ
        outGniazdo.czyBrzegowe = (wykrFace >= 1 And wykrFace <= 4)
        AnalizujScianeCylindrycznaLamelloP14 = True
    End If

    On Error GoTo 0
End Function

' ==============================================================================
' ZAPASOWE SKANOWANIE PETLI WEWNETRZNYCH NA 6 SCIANACH PLASKICH BRYLY (FACE BOUND)
' ==============================================================================
Private Sub SkanujPetlePlaskieLamelloP14( _
    ByVal sf As Object, _
    ByVal L As Double, ByVal W As Double, ByVal T As Double, _
    ByRef wykryte() As DaneGniazdaLamelloP14, _
    ByRef liczbaWykrytych As Long)

    On Error Resume Next
    If sf Is Nothing Then Exit Sub
    Dim b As Object, sciana As Object, lp As Object
    For Each b In sf.Bodies
        For Each sciana In b.Faces
            If CLng(sciana.Type) = 1 Or CBool(sciana.IsPlanar) Then
                If sciana.Loops.Count > 1 Then
                    For Each lp In sciana.Loops
                        Dim lx1 As Double, lx2 As Double, ly1 As Double, ly2 As Double, lz1 As Double, lz2 As Double
                        If PobierzZakresWierzcholkowObiektu(lp, lx1, lx2, ly1, ly2, lz1, lz2) Then
                            Dim dx As Double, dy As Double, dz As Double
                            dx = lx2 - lx1
                            dy = ly2 - ly1
                            dz = lz2 - lz1
                            If CzyWymiaryProstokataToLamelloP14(dx, dy, dz) Then
                                Dim g As DaneGniazdaLamelloP14
                                Dim pasuje As Boolean: pasuje = False
                                ' Powierzchnia gorna (Face 5, w maszynie tylko gorne wrzeciono pobiera frez)
                                If dz <= 1# And (Abs(lz2 - T) <= 1# Or Abs(lz1) <= 1#) Then
                                    g.face = 5: g.z = T
                                    If dy > dx Then g.kierunek = "Y" Else g.kierunek = "X"
                                    g.x = (lx1 + lx2) / 2#
                                    g.y = (ly1 + ly2) / 2#
                                    g.czyBrzegowe = False
                                    pasuje = True
                                ' Brzeg lewy (Face 4) lub prawy (Face 3)
                                ElseIf dx <= 1# And (Abs(lx1) <= 1# Or Abs(lx2 - L) <= 1#) And _
                                       dz >= LAMELLO_STEP_WID_MIN And dz <= LAMELLO_STEP_WID_MAX Then
                                    If Abs(lx1) <= 1# Then
                                        g.face = 4: g.x = 0#
                                    Else
                                        g.face = 3: g.x = L
                                    End If
                                    g.kierunek = "Y"
                                    g.y = (ly1 + ly2) / 2#
                                    g.z = (lz1 + lz2) / 2#
                                    g.czyBrzegowe = True
                                    pasuje = True
                                ' Brzeg przedni (Face 2) lub tylny (Face 1)
                                ElseIf dy <= 1# And (Abs(ly1) <= 1# Or Abs(ly2 - W) <= 1#) And _
                                       dz >= LAMELLO_STEP_WID_MIN And dz <= LAMELLO_STEP_WID_MAX Then
                                    If Abs(ly1) <= 1# Then
                                        g.face = 2: g.y = 0#
                                    Else
                                        g.face = 1: g.y = W
                                    End If
                                    g.kierunek = "X"
                                    g.x = (lx1 + lx2) / 2#
                                    g.z = (lz1 + lz2) / 2#
                                    g.czyBrzegowe = True
                                    pasuje = True
                                End If

                                If pasuje Then
                                    DodajUnikalneGniazdoLamelloP14 wykryte, liczbaWykrytych, g
                                End If
                            End If
                        End If
                    Next lp
                End If
            End If
        Next sciana
    Next b
    On Error GoTo 0
End Sub

' ==============================================================================
' SKANOWANIE WYEKSTRAHOWANYCH SCIEZEK 2D/3D POD KATEM LAMELLO P14
' ==============================================================================
Private Sub SkanujSciezkiLamelloP14( _
    ByVal wszystkieSciezki As Collection, _
    ByVal L As Double, ByVal W As Double, ByVal T As Double, _
    ByRef wykryte() As DaneGniazdaLamelloP14, _
    ByRef liczbaWykrytych As Long)

    On Error Resume Next
    Dim p As Path
    Dim mialemBRep As Boolean
    mialemBRep = (liczbaWykrytych > 0)

    For Each p In wszystkieSciezki
        If Not (p Is Nothing) Then
            Dim cx As Double, cy As Double, cz As Double
            Dim minX As Double, maxX As Double
            Dim minY As Double, maxY As Double
            Dim minZ As Double, maxZ As Double

            ' 1. Zamkniety prostokatny obrys wejscia gniazda (~69.56 x 10.0 mm)
            If p.Closed And p.Elements.Count = 4 Then
                If PobierzWspolrzedneOtworu(p, cx, cy, cz, minX, maxX, minY, maxY, minZ, maxZ) Then
                    Dim dx As Double, dy As Double, dz As Double
                    dx = maxX - minX
                    dy = maxY - minY
                    dz = maxZ - minZ
                    If CzyWymiaryProstokataToLamelloP14(dx, dy, dz) Then
                        Dim g As DaneGniazdaLamelloP14
                        Dim pasuje As Boolean: pasuje = False
                        If dz <= 1# And (Abs(maxZ - T) <= 1# Or Abs(minZ) <= 1#) Then
                            g.face = 5: g.z = T
                            If dy > dx Then g.kierunek = "Y" Else g.kierunek = "X"
                            g.x = (minX + maxX) / 2#
                            g.y = (minY + maxY) / 2#
                            g.czyBrzegowe = False
                            pasuje = True
                        ElseIf dx <= 1# And (Abs(minX) <= 1# Or Abs(maxX - L) <= 1#) And _
                               dz >= LAMELLO_STEP_WID_MIN And dz <= LAMELLO_STEP_WID_MAX Then
                            If Abs(minX) <= 1# Then
                                g.face = 4: g.x = 0#
                            Else
                                g.face = 3: g.x = L
                            End If
                            g.kierunek = "Y"
                            g.y = (minY + maxY) / 2#
                            g.z = (minZ + maxZ) / 2#
                            g.czyBrzegowe = True
                            pasuje = True
                        ElseIf dy <= 1# And (Abs(minY) <= 1# Or Abs(maxY - W) <= 1#) And _
                               dz >= LAMELLO_STEP_WID_MIN And dz <= LAMELLO_STEP_WID_MAX Then
                            If Abs(minY) <= 1# Then
                                g.face = 2: g.y = 0#
                            Else
                                g.face = 1: g.y = W
                            End If
                            g.kierunek = "X"
                            g.x = (minX + maxX) / 2#
                            g.z = (minZ + maxZ) / 2#
                            g.czyBrzegowe = True
                            pasuje = True
                        End If
                        If pasuje Then
                            DodajUnikalneGniazdoLamelloP14 wykryte, liczbaWykrytych, g
                        End If
                    End If
                End If
            End If

            ' 2. Zapasowa konwersja sciezek kolowych tarczy Lamello P14 (D ~ 100.4 mm) wyekstrahowanych przez AutoExtract2
            If Not mialemBRep Then
                Dim dia As Double
                dia = PobierzSredniceOtworu(p)
                If dia >= (LAMELLO_STEP_RAD_MIN * 2#) And dia <= (LAMELLO_STEP_RAD_MAX * 2#) Then
                    If PobierzWspolrzedneOtworu(p, cx, cy, cz, minX, maxX, minY, maxY, minZ, maxZ) Then
                        Dim sDx As Double, sDy As Double, sDz As Double
                        sDx = maxX - minX
                        sDy = maxY - minY
                        sDz = maxZ - minZ

                        Dim vTop As Variant, vBot As Variant
                        vTop = p.Attribute(ATTR_GEO_Z_LEVEL_TOP)
                        vBot = p.Attribute(ATTR_GEO_Z_LEVEL_BOTTOM)
                        Dim maZLevels As Boolean: maZLevels = False
                        Dim zLvlMid As Double, zLvlSpan As Double
                        If Not IsEmpty(vTop) And Not IsEmpty(vBot) Then
                            zLvlSpan = Abs(CDbl(vTop) - CDbl(vBot))
                            If zLvlSpan >= 2# And zLvlSpan <= 16# Then
                                zLvlMid = (CDbl(vTop) + CDbl(vBot)) / 2#
                                maZLevels = True
                            End If
                        End If

                        Dim gCir As DaneGniazdaLamelloP14
                        Dim pasujeCir As Boolean: pasujeCir = False

                        ' 2a. Tarcza pozioma (w plaszczyznie XY) -> gniazdo w brzegu (Face 1..4)
                        If sDz <= 16# And sDx > 40# And sDy > 40# Then
                            Dim zBrzeg As Double
                            If sDz >= LAMELLO_STEP_WID_MIN And sDz <= LAMELLO_STEP_WID_MAX Then
                                zBrzeg = (minZ + maxZ) / 2#
                            ElseIf maZLevels And zLvlMid > 1# And zLvlMid < (T - 1#) Then
                                zBrzeg = zLvlMid
                            ElseIf cz > 1# And cz < (T - 1#) Then
                                zBrzeg = cz
                            Else
                                zBrzeg = T / 2#
                            End If

                            If cx < -10# Then
                                gCir.face = 4: gCir.kierunek = "Y": gCir.x = 0#: gCir.y = cy: gCir.z = zBrzeg: gCir.czyBrzegowe = True
                                pasujeCir = True
                            ElseIf cx > (L + 10#) Then
                                gCir.face = 3: gCir.kierunek = "Y": gCir.x = L: gCir.y = cy: gCir.z = zBrzeg: gCir.czyBrzegowe = True
                                pasujeCir = True
                            ElseIf cy < -10# Then
                                gCir.face = 2: gCir.kierunek = "X": gCir.x = cx: gCir.y = 0#: gCir.z = zBrzeg: gCir.czyBrzegowe = True
                                pasujeCir = True
                            ElseIf cy > (W + 10#) Then
                                gCir.face = 1: gCir.kierunek = "X": gCir.x = cx: gCir.y = W: gCir.z = zBrzeg: gCir.czyBrzegowe = True
                                pasujeCir = True
                            End If

                        ' 2b. Tarcza pionowa w plaszczyznie YZ -> gniazdo od gory (Face 5, Direction="Y")
                        ElseIf sDx <= 16# And sDy > 40# And sDz > 40# Then
                            gCir.face = 5: gCir.kierunek = "Y": gCir.y = cy: gCir.z = T: gCir.czyBrzegowe = False
                            If sDx >= LAMELLO_STEP_WID_MIN And sDx <= LAMELLO_STEP_WID_MAX Then
                                gCir.x = (minX + maxX) / 2#
                            Else
                                gCir.x = cx
                            End If
                            pasujeCir = True

                        ' 2c. Tarcza pionowa w plaszczyznie XZ -> gniazdo od gory (Face 5, Direction="X")
                        ElseIf sDy <= 16# And sDx > 40# And sDz > 40# Then
                            gCir.face = 5: gCir.kierunek = "X": gCir.x = cx: gCir.z = T: gCir.czyBrzegowe = False
                            If sDy >= LAMELLO_STEP_WID_MIN And sDy <= LAMELLO_STEP_WID_MAX Then
                                gCir.y = (minY + maxY) / 2#
                            Else
                                gCir.y = cy
                            End If
                            pasujeCir = True
                        End If

                        If pasujeCir Then
                            DodajUnikalneGniazdoLamelloP14 wykryte, liczbaWykrytych, gCir
                        End If
                    End If
                End If
            End If
        End If
    Next p
    On Error GoTo 0
End Sub

Private Sub DodajUnikalneGniazdoLamelloP14( _
    ByRef wykryte() As DaneGniazdaLamelloP14, _
    ByRef liczbaWykrytych As Long, _
    ByRef nowe As DaneGniazdaLamelloP14)

    Dim i As Long
    For i = 1 To liczbaWykrytych
        If wykryte(i).face = nowe.face And wykryte(i).kierunek = nowe.kierunek Then
            If Abs(wykryte(i).x - nowe.x) <= 1.2 And _
               Abs(wykryte(i).y - nowe.y) <= 1.2 And _
               Abs(wykryte(i).z - nowe.z) <= 1.2 Then
                Exit Sub
            End If
        End If
    Next i

    liczbaWykrytych = liczbaWykrytych + 1
    ReDim Preserve wykryte(1 To liczbaWykrytych)
    wykryte(liczbaWykrytych) = nowe
End Sub

' ==============================================================================
' GENEROWANIE WPISU XML DLA GNIAZDA LAMELLO P14 ORAZ OTWORU TECHNICZNEGO FI 6
' ==============================================================================
Private Sub DodajWpisGniazdaLamelloP14( _
    ByRef g As DaneGniazdaLamelloP14, _
    ByVal L As Double, ByVal W As Double, ByVal T As Double, _
    ByRef linieMaszynXml As Collection, _
    ByRef linieLamelloXml As Collection)

    On Error Resume Next
    Dim szerGniazda As Double
    If g.czyBrzegowe Then
        szerGniazda = LAMELLO_P14_WIDTH_BRZEG ' 10.15 mm pod tarcze w brzegu
    Else
        szerGniazda = LAMELLO_P14_WIDTH_GORA  ' 9.8 mm pod frez od gory/dolu
    End If

    ' 1. Jesli gniazdo jest w brzegu plyty (Face 1, 2, 3, 4), dopisujemy otwor techniczny fi 6 pod imbus na Face 5
    '    Otwor lezy idealnie na srodku gniazda i 7.5 mm od brzegu gniazda formatki,
    '    a jego glebokosc siega do srodka wysokosci gniazda (T - g.z), bez doliczania stozka wiertla.
    If g.czyBrzegowe Then
        Dim otwX As Double, otwY As Double, otwDep As Double
        otwX = g.x
        otwY = g.y

        Select Case g.face
            Case 4 ' Brzeg lewy (X = 0) -> odsunięcie 7.5 mm od lewego brzegu
                otwX = LAMELLO_P14_IMBUS_OFFSET
                otwY = g.y
            Case 3 ' Brzeg prawy (X = L) -> odsunięcie 7.5 mm od prawego brzegu
                otwX = L - LAMELLO_P14_IMBUS_OFFSET
                otwY = g.y
            Case 2 ' Brzeg przedni (Y = 0) -> odsunięcie 7.5 mm od przedniego brzegu
                otwX = g.x
                otwY = LAMELLO_P14_IMBUS_OFFSET
            Case 1 ' Brzeg tylny (Y = W) -> odsunięcie 7.5 mm od tylnego brzegu
                otwX = g.x
                otwY = W - LAMELLO_P14_IMBUS_OFFSET
        End Select

        ' Glebokosc od gornej powierzchni (Face 5, Z = T) do srodka wysokosci gniazda (Z = g.z)
        otwDep = T - g.z
        If otwDep <= 0.5 Or otwDep >= T Then
            otwDep = T / 2#
        End If

        Dim kluczImbus As String
        kluczImbus = "F5_X" & Format$(otwX, "0.0") & "_Y" & Format$(otwY, "0.0") & _
                     "_D" & Format$(LAMELLO_P14_IMBUS_DIA, "0.0")

        Dim xmlImbus As String
        xmlImbus = "          <Machining Type=""2"" IsGenCode=""2"" Face=""5""" & _
                   " X=""" & FormatLiczbaXml(otwX) & """" & _
                   " Y=""" & FormatLiczbaXml(otwY) & """" & _
                   " Diameter=""" & FormatLiczbaXml(LAMELLO_P14_IMBUS_DIA) & """" & _
                   " Depth=""" & FormatLiczbaXml(otwDep) & """ />"

        linieMaszynXml.Add xmlImbus, kluczImbus
    End If

    ' 2. Wpis gniazda Lamello P14 (Type="1002")
    Dim kluczLamello As String
    kluczLamello = "LAMELLO_F" & g.face & "_" & g.kierunek & _
                   "_X" & Format$(g.x, "0.0") & "_Y" & Format$(g.y, "0.0") & "_Z" & Format$(g.z, "0.0")

    Dim xmlLamello As String
    xmlLamello = "          <Machining Type=""1002"" IsGenCode=""2"" Face=""" & g.face & """" & _
                 " Direction=""" & g.kierunek & """" & _
                 " X=""" & FormatLiczbaXml(g.x) & """" & _
                 " Y=""" & FormatLiczbaXml(g.y) & """" & _
                 " Z=""" & FormatLiczbaXml(g.z) & """" & _
                 " Length=""" & FormatLiczbaXml(LAMELLO_P14_LENGTH) & """" & _
                 " Width=""" & FormatLiczbaXml(szerGniazda) & """" & _
                 " MaxDepth=""" & FormatLiczbaXml(LAMELLO_P14_MAX_DEPTH) & """ />"

    linieLamelloXml.Add xmlLamello, kluczLamello
    On Error GoTo 0
End Sub

' ==============================================================================
' SPRAWDZENIE CZY PIONOWY OTWOR JEST OTWOREM TECHNICZNYM FI 6 POD KLUCZ IMBUS
' DLA WYKRYTEGO GNIAZDA LAMELLO P14 W BRZEGU FORMATKI
' ==============================================================================
Private Function CzyOtworToTechnicznyImbusLamelloP14( _
    ByVal pX As Double, ByVal pY As Double, ByVal dia As Double, _
    ByVal L As Double, ByVal W As Double, _
    ByRef wykryteLamello() As DaneGniazdaLamelloP14, _
    ByVal liczbaWykrytychLamello As Long) As Boolean

    CzyOtworToTechnicznyImbusLamelloP14 = False
    If liczbaWykrytychLamello <= 0 Then Exit Function
    If Abs(dia - LAMELLO_P14_IMBUS_DIA) > 0.6 Then Exit Function

    Dim i As Long
    For i = 1 To liczbaWykrytychLamello
        If wykryteLamello(i).czyBrzegowe Then
            Dim otwX As Double, otwY As Double
            otwX = wykryteLamello(i).x
            otwY = wykryteLamello(i).y
            Select Case wykryteLamello(i).face
                Case 4: otwX = LAMELLO_P14_IMBUS_OFFSET: otwY = wykryteLamello(i).y
                Case 3: otwX = L - LAMELLO_P14_IMBUS_OFFSET: otwY = wykryteLamello(i).y
                Case 2: otwX = wykryteLamello(i).x: otwY = LAMELLO_P14_IMBUS_OFFSET
                Case 1: otwX = wykryteLamello(i).x: otwY = W - LAMELLO_P14_IMBUS_OFFSET
            End Select

            If Abs(pX - otwX) <= 1.2 And Abs(pY - otwY) <= 1.2 Then
                CzyOtworToTechnicznyImbusLamelloP14 = True
                Exit Function
            End If
        End If
    Next i
End Function

Private Function CzyWymiaryProstokataToLamelloP14( _
    ByVal dx As Double, ByVal dy As Double, ByVal dz As Double) As Boolean

    CzyWymiaryProstokataToLamelloP14 = False
    ' Jeden wymiar (os wejscia) bliski 0 (<= 1.0 mm), a pozostale dwa to dlugosc (~69.56 mm) i szerokosc (~10.0 mm)
    If dz <= 1# Then
        If (dx >= LAMELLO_STEP_WID_MIN And dx <= LAMELLO_STEP_WID_MAX And _
            dy >= LAMELLO_STEP_LEN_MIN And dy <= LAMELLO_STEP_LEN_MAX) Or _
           (dy >= LAMELLO_STEP_WID_MIN And dy <= LAMELLO_STEP_WID_MAX And _
            dx >= LAMELLO_STEP_LEN_MIN And dx <= LAMELLO_STEP_LEN_MAX) Then
            CzyWymiaryProstokataToLamelloP14 = True
            Exit Function
        End If
    ElseIf dx <= 1# Then
        If (dz >= LAMELLO_STEP_WID_MIN And dz <= LAMELLO_STEP_WID_MAX And _
            dy >= LAMELLO_STEP_LEN_MIN And dy <= LAMELLO_STEP_LEN_MAX) Or _
           (dy >= LAMELLO_STEP_WID_MIN And dy <= LAMELLO_STEP_WID_MAX And _
            dz >= LAMELLO_STEP_LEN_MIN And dz <= LAMELLO_STEP_LEN_MAX) Then
            CzyWymiaryProstokataToLamelloP14 = True
            Exit Function
        End If
    ElseIf dy <= 1# Then
        If (dz >= LAMELLO_STEP_WID_MIN And dz <= LAMELLO_STEP_WID_MAX And _
            dx >= LAMELLO_STEP_LEN_MIN And dx <= LAMELLO_STEP_LEN_MAX) Or _
           (dx >= LAMELLO_STEP_WID_MIN And dx <= LAMELLO_STEP_WID_MAX And _
            dz >= LAMELLO_STEP_LEN_MIN And dz <= LAMELLO_STEP_LEN_MAX) Then
            CzyWymiaryProstokataToLamelloP14 = True
            Exit Function
        End If
    End If
End Function

Private Function PobierzPunkt3D( _
    ByVal ptObj As Object, _
    ByRef outX As Double, ByRef outY As Double, ByRef outZ As Double) As Boolean

    PobierzPunkt3D = False
    If ptObj Is Nothing Then Exit Function
    On Error Resume Next
    Err.Clear
    outX = CDbl(ptObj.x)
    outY = CDbl(ptObj.y)
    outZ = CDbl(ptObj.z)
    If Err.Number <> 0 Then
        Err.Clear
        outX = CDbl(ptObj.xDouble)
        outY = CDbl(ptObj.yDouble)
        outZ = CDbl(ptObj.zDouble)
    End If
    If Err.Number = 0 Then PobierzPunkt3D = True
    On Error GoTo 0
End Function

Private Function PobierzZakresWierzcholkowSciany( _
    ByVal f As Object, _
    ByRef minX As Double, ByRef maxX As Double, _
    ByRef minY As Double, ByRef maxY As Double, _
    ByRef minZ As Double, ByRef maxZ As Double) As Boolean

    PobierzZakresWierzcholkowSciany = PobierzZakresWierzcholkowObiektu(f, minX, maxX, minY, maxY, minZ, maxZ)
End Function

Private Function PobierzZakresWierzcholkowObiektu( _
    ByVal obj As Object, _
    ByRef minX As Double, ByRef maxX As Double, _
    ByRef minY As Double, ByRef maxY As Double, _
    ByRef minZ As Double, ByRef maxZ As Double) As Boolean

    PobierzZakresWierzcholkowObiektu = False
    If obj Is Nothing Then Exit Function
    On Error Resume Next

    minX = 1E+30: maxX = -1E+30
    minY = 1E+30: maxY = -1E+30
    minZ = 1E+30: maxZ = -1E+30
    Dim mamPunkt As Boolean: mamPunkt = False

    Dim v As Object, pt As Object
    Dim px As Double, py As Double, pz As Double
    For Each v In obj.Vertices
        Set pt = Nothing
        Set pt = v.Point
        If PobierzPunkt3D(pt, px, py, pz) Then
            If px < minX Then minX = px
            If px > maxX Then maxX = px
            If py < minY Then minY = py
            If py > maxY Then maxY = py
            If pz < minZ Then minZ = pz
            If pz > maxZ Then maxZ = pz
            mamPunkt = True
        End If
    Next v

    Dim ed As Object
    For Each ed In obj.Edges
        Set pt = Nothing
        If Not (ed.StartVertex Is Nothing) Then
            Set pt = ed.StartVertex.Point
            If PobierzPunkt3D(pt, px, py, pz) Then
                If px < minX Then minX = px
                If px > maxX Then maxX = px
                If py < minY Then minY = py
                If py > maxY Then maxY = py
                If pz < minZ Then minZ = pz
                If pz > maxZ Then maxZ = pz
                mamPunkt = True
            End If
        End If
        Set pt = Nothing
        If Not (ed.EndVertex Is Nothing) Then
            Set pt = ed.EndVertex.Point
            If PobierzPunkt3D(pt, px, py, pz) Then
                If px < minX Then minX = px
                If px > maxX Then maxX = px
                If py < minY Then minY = py
                If py > maxY Then maxY = py
                If pz < minZ Then minZ = pz
                If pz > maxZ Then maxZ = pz
                mamPunkt = True
            End If
        End If

        Dim krok As Long
        For krok = 0 To 2
            Dim pvEv As Object
            Set pvEv = Nothing
            Set pvEv = ed.Evaluate(CDbl(krok) / 2#)
            If Not (pvEv Is Nothing) Then
                If PobierzPunkt3D(pvEv.P, px, py, pz) Then
                    If px < minX Then minX = px
                    If px > maxX Then maxX = px
                    If py < minY Then minY = py
                    If py > maxY Then maxY = py
                    If pz < minZ Then minZ = pz
                    If pz > maxZ Then maxZ = pz
                    mamPunkt = True
                End If
            End If
        Next krok
    Next ed

    If Not mamPunkt Then
        Dim bnd As Object
        Set bnd = Nothing
        Set bnd = obj.Bounds
        If Not (bnd Is Nothing) Then
            Dim bx As Double, by As Double, bz As Double
            Dim vx As Double, vy As Double, vz As Double
            If PobierzPunkt3D(bnd.P, bx, by, bz) Then
                Err.Clear
                vx = CDbl(bnd.V.dx): vy = CDbl(bnd.V.dy): vz = CDbl(bnd.V.dz)
                If Err.Number <> 0 Then
                    Err.Clear
                    vx = CDbl(bnd.V.dxDouble): vy = CDbl(bnd.V.dyDouble): vz = CDbl(bnd.V.dzDouble)
                End If
                If Err.Number = 0 Then
                    If bx < (bx + vx) Then minX = bx: maxX = bx + vx Else minX = bx + vx: maxX = bx
                    If by < (by + vy) Then minY = by: maxY = by + vy Else minY = by + vy: maxY = by
                    If bz < (bz + vz) Then minZ = bz: maxZ = bz + vz Else minZ = bz + vz: maxZ = bz
                    mamPunkt = True
                End If
            End If
        End If
    End If

    If mamPunkt And minX <= maxX And minY <= maxY And minZ <= maxZ Then
        PobierzZakresWierzcholkowObiektu = True
    End If
    On Error GoTo 0
End Function

' ==============================================================================
' WERYFIKACJA OTWORU W GEOMETRII B-REP 3D:
' 1. Sprawdzenie czy otwor jest pelnym cylindrem 360 st. (odrzucenie promieni R w naroznikach podciec,
'    gdy uwzgledniajCzesciowe = False).
' 2. Znalezienie rzeczywistego dna otworu w modelu 3D i wyliczenie glebokosci od powierzchni wejscia.
' 3. Dla otworow poziomych bez plaskiego/kolowego dna (wpadajacych w otwor pionowy np. mimosrodu fi 15)
'    wyliczenie glebokosci z zakresu 3D sciany cylindrycznej + 2 mm naddatku na pewne przebicie.
' ==============================================================================
Private Function WeryfikujOtworZBRep( _
    ByVal sf As Object, _
    ByVal tTyp As Long, _
    ByRef ioFace As Long, _
    ByVal pX As Double, ByVal pY As Double, ByVal pZ As Double, _
    ByVal dia As Double, _
    ByVal L As Double, ByVal W As Double, ByVal T As Double, _
    ByVal uwzgledniajCzesciowe As Boolean, _
    ByRef ioDep As Double) As Boolean

    WeryfikujOtworZBRep = True
    If sf Is Nothing Then Exit Function
    On Error Resume Next
    If sf.Bodies.Count = 0 Then Exit Function

    Dim combMinX As Double, combMaxX As Double
    Dim combMinY As Double, combMaxY As Double
    Dim combMinZ As Double, combMaxZ As Double
    combMinX = 1E+30: combMaxX = -1E+30
    combMinY = 1E+30: combMaxY = -1E+30
    combMinZ = 1E+30: combMaxZ = -1E+30

    Dim circMinX As Double, circMaxX As Double
    Dim circMinY As Double, circMaxY As Double
    Dim circMinZ As Double, circMaxZ As Double
    circMinX = 1E+30: circMaxX = -1E+30
    circMinY = 1E+30: circMaxY = -1E+30
    circMinZ = 1E+30: circMaxZ = -1E+30
    Dim mamCirc As Boolean: mamCirc = False

    Dim znalezionoSciane As Boolean: znalezionoSciane = False
    Dim dRef As Double: dRef = dia

    Dim b As Object, f As Object
    For Each b In sf.Bodies
        For Each f In b.Faces
            If CLng(f.Type) = 2 Then
                Dim c As Object
                Set c = f.Conic
                If Not (c Is Nothing) Then
                    If CBool(c.Hollow) Then
                        Dim rad As Double
                        rad = 0#
                        Err.Clear
                        rad = CDbl(c.Radius.Value)
                        If Err.Number <> 0 Or rad <= 0# Then
                            Err.Clear
                            rad = CDbl(c.Radius)
                        End If
                        Dim dCyl As Double
                        dCyl = rad * 2#

                        If Abs(dCyl - dia) <= 1.3 Then
                            ' Odczyt osi i kierunku cylindra
                            Dim ax As Object
                            Set ax = c.Axis
                            Dim axX As Double, axY As Double, axZ As Double
                            axX = 0#: axY = 0#: axZ = 0#
                            Dim apx As Double, apy As Double, apz As Double
                            Dim hasAxisPt As Boolean: hasAxisPt = False

                            If Not (ax Is Nothing) Then
                                If Not (ax.P Is Nothing) Then
                                    If PobierzPunkt3D(ax.P, apx, apy, apz) Then hasAxisPt = True
                                End If
                                Err.Clear
                                If Not (ax.V Is Nothing) Then
                                    axX = Abs(CDbl(ax.V.dx))
                                    axY = Abs(CDbl(ax.V.dy))
                                    axZ = Abs(CDbl(ax.V.dz))
                                    If Err.Number <> 0 Then
                                        Err.Clear
                                        axX = Abs(CDbl(ax.V.dxDouble))
                                        axY = Abs(CDbl(ax.V.dyDouble))
                                        axZ = Abs(CDbl(ax.V.dzDouble))
                                    End If
                                End If
                                If (axX + axY + axZ) < 0.0001 Then
                                    Err.Clear
                                    axX = Abs(CDbl(ax.dx))
                                    axY = Abs(CDbl(ax.dy))
                                    axZ = Abs(CDbl(ax.dz))
                                End If
                            End If

                            ' Odczyt krawedzi kolowych tej sciany
                            Dim fCircMinX As Double, fCircMaxX As Double
                            Dim fCircMinY As Double, fCircMaxY As Double
                            Dim fCircMinZ As Double, fCircMaxZ As Double
                            Dim fHasCirc As Boolean: fHasCirc = False
                            Dim edg As Object
                            For Each edg In f.Edges
                                If CLng(edg.Type) = 2 Then
                                    Dim ecx As Double, ecy As Double, ecz As Double
                                    If PobierzPunkt3D(edg.Center, ecx, ecy, ecz) Then
                                        If Not fHasCirc Then
                                            fCircMinX = ecx: fCircMaxX = ecx
                                            fCircMinY = ecy: fCircMaxY = ecy
                                            fCircMinZ = ecz: fCircMaxZ = ecz
                                            fHasCirc = True
                                        Else
                                            If ecx < fCircMinX Then fCircMinX = ecx
                                            If ecx > fCircMaxX Then fCircMaxX = ecx
                                            If ecy < fCircMinY Then fCircMinY = ecy
                                            If ecy > fCircMaxY Then fCircMaxY = ecy
                                            If ecz < fCircMinZ Then fCircMinZ = ecz
                                            If ecz > fCircMaxZ Then fCircMaxZ = ecz
                                        End If
                                    End If
                                End If
                            Next edg

                            Dim axisX As Double, axisY As Double, axisZ As Double
                            Dim mamOs As Boolean: mamOs = False
                            If fHasCirc Then
                                axisX = (fCircMinX + fCircMaxX) / 2#
                                axisY = (fCircMinY + fCircMaxY) / 2#
                                axisZ = (fCircMinZ + fCircMaxZ) / 2#
                                mamOs = True
                            ElseIf hasAxisPt Then
                                axisX = apx: axisY = apy: axisZ = apz
                                mamOs = True
                            End If

                            If mamOs Then
                                Dim vx1 As Double, vx2 As Double, vy1 As Double, vy2 As Double, vz1 As Double, vz2 As Double
                                Dim hasV As Boolean
                                hasV = PobierzZakresWierzcholkowSciany(f, vx1, vx2, vy1, vy2, vz1, vz2)

                                Dim pasujeSciana As Boolean: pasujeSciana = False
                                If tTyp = 2 Then
                                    ' Otwor pionowy (os Z)
                                    If axZ > 0.7 Or (axZ >= axX And axZ >= axY) Then
                                        If Abs(axisX - pX) <= 1.5 And Abs(axisY - pY) <= 1.5 Then
                                            pasujeSciana = True
                                        End If
                                    End If
                                ElseIf tTyp = 1 Then
                                    If ioFace = 1 Or ioFace = 2 Then
                                        ' Otwor poziomy w osi Y
                                        If axY > 0.7 Or (axY > axX And axY > axZ) Then
                                            If Abs(axisX - pX) <= 1.5 And Abs(axisZ - pZ) <= 1.5 Then
                                                If Not hasV Then
                                                    pasujeSciana = True
                                                ElseIf ioFace = 2 And vy1 <= 2.5 Then
                                                    pasujeSciana = True
                                                ElseIf ioFace = 1 And vy2 >= (W - 2.5) Then
                                                    pasujeSciana = True
                                                End If
                                            End If
                                        End If
                                    ElseIf ioFace = 3 Or ioFace = 4 Then
                                        ' Otwor poziomy w osi X
                                        If axX > 0.7 Or (axX > axY And axX > axZ) Then
                                            If Abs(axisY - pY) <= 1.5 And Abs(axisZ - pZ) <= 1.5 Then
                                                If Not hasV Then
                                                    pasujeSciana = True
                                                ElseIf ioFace = 4 And vx1 <= 2.5 Then
                                                    pasujeSciana = True
                                                ElseIf ioFace = 3 And vx2 >= (L - 2.5) Then
                                                    pasujeSciana = True
                                                End If
                                            End If
                                        End If
                                    End If
                                End If

                                If pasujeSciana Then
                                    znalezionoSciane = True
                                    dRef = dCyl
                                    If hasV Then
                                        If vx1 < combMinX Then combMinX = vx1
                                        If vx2 > combMaxX Then combMaxX = vx2
                                        If vy1 < combMinY Then combMinY = vy1
                                        If vy2 > combMaxY Then combMaxY = vy2
                                        If vz1 < combMinZ Then combMinZ = vz1
                                        If vz2 > combMaxZ Then combMaxZ = vz2
                                    End If
                                    If fHasCirc Then
                                        If fCircMinX < circMinX Then circMinX = fCircMinX
                                        If fCircMaxX > circMaxX Then circMaxX = fCircMaxX
                                        If fCircMinY < circMinY Then circMinY = fCircMinY
                                        If fCircMaxY > circMaxY Then circMaxY = fCircMaxY
                                        If fCircMinZ < circMinZ Then circMinZ = fCircMinZ
                                        If fCircMaxZ > circMaxZ Then circMaxZ = fCircMaxZ
                                        mamCirc = True

                                        If fCircMinZ < combMinZ Then combMinZ = fCircMinZ
                                        If fCircMaxZ > combMaxZ Then combMaxZ = fCircMaxZ
                                    End If
                                End If
                            End If
                        End If
                    End If
                End If
            End If
        Next f
    Next b

    If Not znalezionoSciane Then
        On Error GoTo 0
        Exit Function
    End If

    ' 1. Test pelnego obwodu otworu 360 st. (gdy uwzgledniajCzesciowe = False)
    If Not uwzgledniajCzesciowe And combMinX <= combMaxX Then
        Dim spanPerp1 As Double, spanPerp2 As Double
        If tTyp = 2 Then
            spanPerp1 = combMaxX - combMinX
            spanPerp2 = combMaxY - combMinY
        ElseIf ioFace = 1 Or ioFace = 2 Then
            spanPerp1 = combMaxX - combMinX
            spanPerp2 = combMaxZ - combMinZ
        Else
            spanPerp1 = combMaxY - combMinY
            spanPerp2 = combMaxZ - combMinZ
        End If

        ' Dla promienia w narozniku podciecia (np. R=12, D=24, kat 90 st.) rozpietosc wynosi tylko 12x12 mm (0.5 * D).
        ' Pelny otwor 360 st. (1 sciana lub 2 polowki 180 st.) ma rozpietosc ~1.0 * D w obu osiach prostopadlych.
        If spanPerp1 < (dRef * 0.82) Or spanPerp2 < (dRef * 0.82) Then
            WeryfikujOtworZBRep = False
            On Error GoTo 0
            Exit Function
        End If
    End If

    ' 2. Wyznaczenie rzeczywistej glebokosci od powierzchni wejscia do dna otworu w modelu 3D
    If tTyp = 2 And combMinZ <= combMaxZ Then
        ' Otwor przelotowy od Z=0 do Z=T
        If combMinZ <= 0.5 And combMaxZ >= (T - 0.5) Then
            ioDep = T
        ' Otwor nieprzelotowy od gory (Face 5, Z = T) -> dno otworu znajduje sie na Z = combMinZ
        ElseIf combMaxZ >= (T - 0.8) And combMinZ > 0.5 Then
            ioDep = T - combMinZ
            ioFace = 5
        ' Otwor nieprzelotowy od dolu (Z = 0) -> dno otworu znajduje sie na Z = combMaxZ
        ElseIf combMinZ <= 0.8 And combMaxZ < (T - 0.5) Then
            ioDep = combMaxZ
        ElseIf (combMaxZ - combMinZ) > 0.5 Then
            ioDep = combMaxZ - combMinZ
        End If

    ElseIf tTyp = 1 And combMinX <= combMaxX Then
        Dim vDepBRep As Double: vDepBRep = 0#
        Dim cSpanBRep As Double: cSpanBRep = 0#

        Select Case ioFace
            Case 2 ' Przod (Y = 0 -> w glab +Y)
                vDepBRep = combMaxY
                If mamCirc Then cSpanBRep = circMaxY - circMinY
            Case 1 ' Tyl (Y = W -> w glab -Y)
                vDepBRep = W - combMinY
                If mamCirc Then cSpanBRep = circMaxY - circMinY
            Case 4 ' Lewo (X = 0 -> w glab +X)
                vDepBRep = combMaxX
                If mamCirc Then cSpanBRep = circMaxX - circMinX
            Case 3 ' Prawo (X = L -> w glab -X)
                vDepBRep = L - combMinX
                If mamCirc Then cSpanBRep = circMaxX - circMinX
        End Select

        If vDepBRep > 1# Then
            ' Jesli otwor poziomy nie posiada kolowej krawedzi dna na koncu (cSpanBRep <= 0.8 lub vDepBRep > cSpanBRep + 0.8),
            ' oznacza to otwor poziomy bez dna (np. laczacy sie bezposrednio z otworem mimosrodu fi 15).
            ' Dodajemy +2 mm do wyliczonej glebokosci 3D, aby wiertlo na pewno przebilo sie do otworu pionowego.
            If cSpanBRep <= 0.8 Or vDepBRep > (cSpanBRep + 0.8) Then
                ioDep = vDepBRep + NADDANEK_OTWORU_POZIOMEGO_BEZ_DNA
            Else
                ioDep = vDepBRep
            End If
        End If
    End If

    On Error GoTo 0
End Function

' ==============================================================================
' BEZPOSREDNIA DETEKCJA OTWOROW Z ELEMENTOW GEOMETRII BRYLY SOLID (B-REP)
' Zabezpiecza przed sytuacja, gdy silnik ekstrakcji Alphacam pominie wiercenia
' na bocznych krawedziach trudnych modeli STEP.
' ==============================================================================
Private Sub EkstrahujOtworyDirectZSolidFaces( _
    ByVal sf As Object, _
    ByVal L As Double, ByVal W As Double, ByVal T As Double, _
    ByVal zamienGzymki As Boolean, _
    ByVal kompensujStozek As Boolean, _
    ByVal uwzgledniajCzesciowe As Boolean, _
    ByRef wykryteLamello() As DaneGniazdaLamelloP14, _
    ByVal liczbaWykrytychLamello As Long, _
    ByRef linieMaszynXml As Collection)

    On Error Resume Next
    If sf Is Nothing Then Exit Sub
    If sf.Bodies.Count = 0 Then Exit Sub

    Dim b As Object
    For Each b In sf.Bodies
        Dim f As Object
        For Each f In b.Faces
            ' Typ 2 = FeatureCylindricalFace (powierzchnia cylindryczna)
            If f.Type = 2 Then
                Dim c As Object
                Set c = f.Conic
                If Not (c Is Nothing) Then
                    If c.Hollow Then
                        Dim rad As Double
                        rad = 0#
                        Err.Clear
                        rad = CDbl(c.Radius.Value)
                        If Err.Number <> 0 Or rad <= 0# Then
                            Err.Clear
                            rad = CDbl(c.Radius)
                        End If
                        Dim dia As Double
                        dia = rad * 2#
                        Dim diaOryg As Double
                        diaOryg = dia

                        ' Pomijamy powierzchnie cylindryczne tarczy Lamello P14 (~100.4 mm) oraz duze wycięcia > 45 mm
                        If dia > 0.5 And dia <= 45# And Not (rad >= LAMELLO_STEP_RAD_MIN And rad <= LAMELLO_STEP_RAD_MAX) Then
                            If zamienGzymki Then
                                If Abs(dia - 11#) < 0.6 Or Abs(dia - 10.8) < 0.4 Then
                                    dia = 12#
                                End If
                            End If

                            ' Analiza wektora osi cylindra (zarowno c.Axis.V jak i bezposrednio c.Axis)
                            Dim ax As Object
                            Set ax = c.Axis
                            Dim axX As Double, axY As Double, axZ As Double
                            axX = 0#: axY = 0#: axZ = 0#
                            If Not (ax Is Nothing) Then
                                Err.Clear
                                If Not (ax.V Is Nothing) Then
                                    axX = Abs(CDbl(ax.V.dx))
                                    axY = Abs(CDbl(ax.V.dy))
                                    axZ = Abs(CDbl(ax.V.dz))
                                    If Err.Number <> 0 Then
                                        Err.Clear
                                        axX = Abs(CDbl(ax.V.dxDouble))
                                        axY = Abs(CDbl(ax.V.dyDouble))
                                        axZ = Abs(CDbl(ax.V.dzDouble))
                                    End If
                                End If
                                If (axX + axY + axZ) < 0.0001 Then
                                    Err.Clear
                                    axX = Abs(CDbl(ax.dx))
                                    axY = Abs(CDbl(ax.dy))
                                    axZ = Abs(CDbl(ax.dz))
                                End If
                            End If

                            ' Analiza krawedzi cylindra w celu wyznaczenia pozycji i glebokosci
                            Dim minEdgeX As Double, maxEdgeX As Double
                            Dim minEdgeY As Double, maxEdgeY As Double
                            Dim minEdgeZ As Double, maxEdgeZ As Double
                            Dim firstEdge As Boolean: firstEdge = True
                            Dim hasCircEdge As Boolean: hasCircEdge = False
                            Dim edg As Object

                            For Each edg In f.Edges
                                If edg.Type = 2 Then ' FeatureCircularEdge = 2
                                    Dim cpt As Object
                                    Set cpt = edg.Center
                                    Dim ecx As Double, ecy As Double, ecz As Double
                                    If PobierzPunkt3D(cpt, ecx, ecy, ecz) Then
                                        hasCircEdge = True
                                        If firstEdge Then
                                            minEdgeX = ecx: maxEdgeX = ecx
                                            minEdgeY = ecy: maxEdgeY = ecy
                                            minEdgeZ = ecz: maxEdgeZ = ecz
                                            firstEdge = False
                                        Else
                                            If ecx < minEdgeX Then minEdgeX = ecx
                                            If ecx > maxEdgeX Then maxEdgeX = ecx
                                            If ecy < minEdgeY Then minEdgeY = ecy
                                            If ecy > maxEdgeY Then maxEdgeY = ecy
                                            If ecz < minEdgeZ Then minEdgeZ = ecz
                                            If ecz > maxEdgeZ Then maxEdgeZ = ecz
                                        End If
                                    End If
                                End If
                            Next edg

                            If hasCircEdge Then
                                Dim tTyp As Long, tFace As Long
                                Dim pX As Double, pY As Double, pZ As Double
                                Dim dep As Double: dep = 0#

                                ' 1. Otwor poziomy w osi Y (Boki Face 1 / Face 2)
                                If axY > 0.7 Or (axY > axX And axY > axZ) Then
                                    tTyp = 1
                                    pX = (minEdgeX + maxEdgeX) / 2#
                                    pZ = (minEdgeZ + maxEdgeZ) / 2#
                                    dep = maxEdgeY - minEdgeY
                                    If dep <= 0.05 Then dep = 18#
                                    If dep > W Then dep = W

                                    If minEdgeY < (W - maxEdgeY) Then
                                        tFace = 2
                                        pY = 0#
                                    Else
                                        tFace = 1
                                        pY = W
                                    End If

                                ' 2. Otwor poziomy w osi X (Boki Face 3 / Face 4)
                                ElseIf axX > 0.7 Or (axX > axY And axX > axZ) Then
                                    tTyp = 1
                                    pY = (minEdgeY + maxEdgeY) / 2#
                                    pZ = (minEdgeZ + maxEdgeZ) / 2#
                                    dep = maxEdgeX - minEdgeX
                                    If dep <= 0.05 Then dep = 25#
                                    If dep > L Then dep = L

                                    If minEdgeX < (L - maxEdgeX) Then
                                        tFace = 4
                                        pX = 0#
                                    Else
                                        tFace = 3
                                        pX = L
                                    End If

                                ' 3. Otwor pionowy w osi Z (Gora Face 5 / Dol Face 6)
                                Else
                                    tTyp = 2
                                    pX = (minEdgeX + maxEdgeX) / 2#
                                    pY = (minEdgeY + maxEdgeY) / 2#
                                    pZ = 0#
                                    dep = maxEdgeZ - minEdgeZ
                                    If dep <= 0.05 Or dep > T Then dep = T
                                    tFace = 5
                                    If maxEdgeZ < (T / 2#) Then tFace = 6
                                End If

                                ' Ograniczenie do granic formatki
                                If pX < 0# Then pX = 0#
                                If pX > L Then pX = L
                                If pY < 0# Then pY = 0#
                                If pY > W Then pY = W
                                If pZ < 0# Then pZ = 0#
                                If pZ > T Then pZ = T

                                Dim czyTechnicznyImbus As Boolean
                                czyTechnicznyImbus = False
                                If tTyp = 2 Then
                                    czyTechnicznyImbus = CzyOtworToTechnicznyImbusLamelloP14(pX, pY, dia, L, W, wykryteLamello, liczbaWykrytychLamello)
                                End If

                                ' Weryfikacja pelnego obwodu 360 st. oraz rzeczywistej glebokosci do dna otworu (w tym +2 mm dla poziomych bez dna)
                                Dim czyPoprawnyOtworBRep As Boolean
                                czyPoprawnyOtworBRep = False
                                If Not czyTechnicznyImbus Then
                                    czyPoprawnyOtworBRep = WeryfikujOtworZBRep(sf, tTyp, tFace, pX, pY, pZ, diaOryg, L, W, T, uwzgledniajCzesciowe, dep)
                                End If

                                If Not czyTechnicznyImbus And czyPoprawnyOtworBRep Then
                                    ' Kompensacja stozka wiertla
                                    If kompensujStozek And dia <= 12.5 Then
                                        Dim czyNieprzelotowy As Boolean
                                        czyNieprzelotowy = False
                                        If tTyp = 2 Then
                                            If dep < (T - 0.2) Then czyNieprzelotowy = True
                                        ElseIf tTyp = 1 Then
                                            If (tFace = 1 Or tFace = 2) And dep < (W - 0.5) Then czyNieprzelotowy = True
                                            If (tFace = 3 Or tFace = 4) And dep < (L - 0.5) Then czyNieprzelotowy = True
                                        End If
                                        If czyNieprzelotowy Then
                                            dep = dep + ((dia / 2#) / Tan(1.02974425867665))
                                        End If
                                    End If

                                    ' Bezwzgledny limit maksymalnej glebokosci wiercenia bocznego (sciany 1..4) do 50 mm
                                    If tTyp = 1 Then
                                        If dep > MAX_GLEBOKOSC_WIERCENIA_BOCZNEGO Then
                                            dep = MAX_GLEBOKOSC_WIERCENIA_BOCZNEGO
                                        End If
                                    End If

                                    ' Klucz unikalny
                                    Dim kluczDirect As String
                                    If tTyp = 2 Then
                                        kluczDirect = "F" & tFace & "_X" & Format$(pX, "0.0") & "_Y" & Format$(pY, "0.0") & "_D" & Format$(dia, "0.0")
                                    Else
                                        kluczDirect = "F" & tFace & "_X" & Format$(pX, "0.0") & "_Y" & Format$(pY, "0.0") & "_Z" & Format$(pZ, "0.0") & "_D" & Format$(dia, "0.0")
                                    End If

                                    ' Linia XML
                                    Dim xmlDirect As String
                                    If tTyp = 1 Then
                                        xmlDirect = "          <Machining Type=""1"" IsGenCode=""2"" Face=""" & tFace & """" & _
                                                    " X=""" & FormatLiczbaXml(pX) & """" & _
                                                    " Y=""" & FormatLiczbaXml(pY) & """" & _
                                                    " Z=""" & FormatLiczbaXml(pZ) & """" & _
                                                    " Diameter=""" & FormatLiczbaXml(dia) & """" & _
                                                    " Depth=""" & FormatLiczbaXml(dep) & """ />"
                                    Else
                                        xmlDirect = "          <Machining Type=""2"" IsGenCode=""2"" Face=""" & tFace & """" & _
                                                    " X=""" & FormatLiczbaXml(pX) & """" & _
                                                    " Y=""" & FormatLiczbaXml(pY) & """" & _
                                                    " Diameter=""" & FormatLiczbaXml(dia) & """" & _
                                                    " Depth=""" & FormatLiczbaXml(dep) & """ />"
                                    End If

                                    On Error Resume Next
                                    linieMaszynXml.Add xmlDirect, kluczDirect
                                    On Error GoTo 0
                                End If
                            End If
                        End If
                    End If
                End If
            End If
        Next f
    Next b
    On Error GoTo 0
End Sub

' ==============================================================================
' FUNKCJE POMOCNICZE (FORMATOWANIE, ATRYBUTY, ZAPIS UTF-8)
' ==============================================================================
Private Function FormatLiczbaXml(ByVal val As Double) As String
    If Abs(val - Round(val)) < 0.0001 Then
        FormatLiczbaXml = CStr(CLng(Round(val)))
    Else
        Dim s As String
        s = Format$(val, "0.####")
        FormatLiczbaXml = Replace(s, ",", ".")
    End If
End Function

Private Sub ZapiszPlikUtf8(ByVal sciezka As String, ByVal tekst As String)
    Dim stm As Object
    Set stm = CreateObject("ADODB.Stream")
    stm.Type = 2
    stm.Charset = "utf-8"
    stm.Open
    stm.WriteText tekst
    stm.SaveToFile sciezka, 2
    stm.Close
End Sub

Private Function CzySciezkaToOtwor(ByVal p As Path) As Boolean
    CzySciezkaToOtwor = False
    If p Is Nothing Then Exit Function

    On Error Resume Next
    If p.IsDrilling Then
        CzySciezkaToOtwor = True
        Exit Function
    End If

    Dim vDia As Variant
    vDia = p.Attribute(ATTR_HOLE_DIAMETER)
    If Not IsEmpty(vDia) And Not IsNull(vDia) Then
        If CDbl(vDia) > 0# Then
            CzySciezkaToOtwor = True
            Exit Function
        End If
    End If

    If Not (p.GetCircleProperties Is Nothing) Then
        CzySciezkaToOtwor = True
        Exit Function
    End If

    If Not (p.GetLayer Is Nothing) Then
        Dim uLyr As String
        uLyr = UCase$(p.GetLayer.Name)
        If InStr(1, uLyr, "HOLE", vbTextCompare) > 0 Or _
           InStr(1, uLyr, "OTWOR", vbTextCompare) > 0 Or _
           InStr(1, uLyr, "DRILL", vbTextCompare) > 0 Then
            CzySciezkaToOtwor = True
            Exit Function
        End If
    End If

    ' Sprawdzenie czy sciezka zamknieta sklada sie z samych lukow (okrag na sciance bocznej)
    If p.Closed And p.Elements.Count >= 1 And p.Elements.Count <= 4 Then
        Dim wElem As Element
        Dim allArcs As Boolean
        allArcs = True
        For Each wElem In p.Elements
            If Not wElem.IsArc Then
                allArcs = False
                Exit For
            End If
        Next wElem
        If allArcs Then
            CzySciezkaToOtwor = True
            Exit Function
        End If
    End If
    On Error GoTo 0
End Function

Private Function PobierzSredniceOtworu(ByVal p As Path) As Double
    PobierzSredniceOtworu = 0#
    If p Is Nothing Then Exit Function

    On Error Resume Next
    Dim vDia As Variant
    vDia = p.Attribute(ATTR_HOLE_DIAMETER)
    If Not IsEmpty(vDia) And Not IsNull(vDia) Then
        PobierzSredniceOtworu = CDbl(vDia)
        If PobierzSredniceOtworu > 0# Then Exit Function
    End If

    If Not (p.GetCircleProperties Is Nothing) Then
        PobierzSredniceOtworu = p.GetCircleProperties.Radius * 2#
        If PobierzSredniceOtworu > 0# Then Exit Function
    End If

    Dim sCx As Double, sCy As Double, sCz As Double
    Dim sMinX As Double, sMaxX As Double
    Dim sMinY As Double, sMaxY As Double
    Dim sMinZ As Double, sMaxZ As Double
    If PobierzWspolrzedneOtworu(p, sCx, sCy, sCz, sMinX, sMaxX, sMinY, sMaxY, sMinZ, sMaxZ) Then
        Dim bdx As Double, bdy As Double, bdz As Double
        bdx = sMaxX - sMinX
        bdy = sMaxY - sMinY
        bdz = sMaxZ - sMinZ

        If bdx >= bdy And bdx >= bdz Then
            PobierzSredniceOtworu = bdx
        ElseIf bdy >= bdz Then
            PobierzSredniceOtworu = bdy
        Else
            PobierzSredniceOtworu = bdz
        End If
    End If
    On Error GoTo 0
End Function

Private Function PobierzGlebokoscOtworu(ByVal p As Path) As Double
    PobierzGlebokoscOtworu = 0#
    If p Is Nothing Then Exit Function

    On Error Resume Next
    ' Najpierw rzeczywista roznica poziomow gora-dol otworu (odporna na przesuniecie bazy Z=0 na spod formatki)
    Dim vTop As Variant, vBot As Variant
    vTop = p.Attribute(ATTR_GEO_Z_LEVEL_TOP)
    vBot = p.Attribute(ATTR_GEO_Z_LEVEL_BOTTOM)
    If Not IsEmpty(vTop) And Not IsEmpty(vBot) Then
        PobierzGlebokoscOtworu = Abs(CDbl(vTop) - CDbl(vBot))
        If PobierzGlebokoscOtworu > 0.05 Then Exit Function
    End If

    Dim vDep As Variant
    vDep = p.Attribute("LicomUKRJMHoleDepth")
    If Not IsEmpty(vDep) And Not IsNull(vDep) Then
        PobierzGlebokoscOtworu = Abs(CDbl(vDep))
        If PobierzGlebokoscOtworu > 0.05 Then Exit Function
    End If

    vDep = p.Attribute("HoleDepth")
    If Not IsEmpty(vDep) And Not IsNull(vDep) Then
        PobierzGlebokoscOtworu = Abs(CDbl(vDep))
        If PobierzGlebokoscOtworu > 0.05 Then Exit Function
    End If

    vDep = p.Attribute("Depth")
    If Not IsEmpty(vDep) And Not IsNull(vDep) Then
        PobierzGlebokoscOtworu = Abs(CDbl(vDep))
        If PobierzGlebokoscOtworu > 0.05 Then Exit Function
    End If

    vDep = p.Attribute("LicomUKRJMHoleShoulder")
    If Not IsEmpty(vDep) And Not IsNull(vDep) Then
        PobierzGlebokoscOtworu = Abs(CDbl(vDep))
        If PobierzGlebokoscOtworu > 0.05 Then Exit Function
    End If
    On Error GoTo 0
End Function

Private Sub DodajSciezkeUnikalna(ByVal p As Path, ByRef kol As Collection)
    If p Is Nothing Then Exit Sub
    On Error Resume Next
    Dim istn As Path
    For Each istn In kol
        If istn.IsSame(p) Then Exit Sub
    Next istn
    kol.Add p
    On Error GoTo 0
End Sub

' ==============================================================================
' OBSLUGA BRYL SOLID
' ==============================================================================
Private Function LiczbaBrylWRysunku(ByVal drw As Drawing, ByVal sf As Object) As Long
    On Error Resume Next
    Dim cnt As Long
    cnt = 0
    If Not (sf Is Nothing) Then
        cnt = sf.Bodies.Count
    End If
    If cnt = 0 And Not (drw Is Nothing) Then
        cnt = drw.SolidParts.Count
    End If
    LiczbaBrylWRysunku = cnt
    On Error GoTo 0
End Function

Private Sub PobierzGabarytyBryly( _
    ByVal drw As Drawing, _
    ByVal sf As Object, _
    ByRef minX As Double, ByRef maxX As Double, _
    ByRef minY As Double, ByRef maxY As Double, _
    ByRef minZ As Double, ByRef maxZ As Double)

    On Error Resume Next
    Dim first As Boolean
    first = True

    If Not (sf Is Nothing) Then
        If sf.Bodies.Count > 0 Then
            Dim b As Object
            For Each b In sf.Bodies
                If first Then
                    minX = CDbl(b.MinX): maxX = CDbl(b.MaxX)
                    minY = CDbl(b.MinY): maxY = CDbl(b.MaxY)
                    minZ = CDbl(b.MinZ): maxZ = CDbl(b.MaxZ)
                    first = False
                Else
                    If CDbl(b.MinX) < minX Then minX = CDbl(b.MinX)
                    If CDbl(b.MaxX) > maxX Then maxX = CDbl(b.MaxX)
                    If CDbl(b.MinY) < minY Then minY = CDbl(b.MinY)
                    If CDbl(b.MaxY) > maxY Then maxY = CDbl(b.MaxY)
                    If CDbl(b.MinZ) < minZ Then minZ = CDbl(b.MinZ)
                    If CDbl(b.MaxZ) > maxZ Then maxZ = CDbl(b.MaxZ)
                End If
            Next b
            On Error GoTo 0
            Exit Sub
        End If
    End If

    Dim sp As Object
    For Each sp In drw.SolidParts
        If first Then
            minX = sp.MinX: maxX = sp.MaxX
            minY = sp.MinY: maxY = sp.MaxY
            minZ = sp.MinZ: maxZ = sp.MaxZ
            first = False
        Else
            If sp.MinX < minX Then minX = sp.MinX
            If sp.MaxX > maxX Then maxX = sp.MaxX
            If sp.MinY < minY Then minY = sp.MinY
            If sp.MaxY > maxY Then maxY = sp.MaxY
            If sp.MinZ < minZ Then minZ = sp.MinZ
            If sp.MaxZ > maxZ Then maxZ = sp.MaxZ
        End If
    Next sp
    On Error GoTo 0
End Sub

Private Sub PrzesunWszystkieBryly(ByVal drw As Drawing, ByVal sf As Object, ByVal dx As Double, ByVal dy As Double, ByVal dz As Double)
    On Error Resume Next
    If Not (sf Is Nothing) Then
        If sf.Bodies.Count > 0 Then
            Dim bItem As Object
            For Each bItem In sf.Bodies
                bItem.MoveG dx, dy, dz
            Next bItem
            On Error GoTo 0
            Exit Sub
        End If
    End If

    Dim sp As Object
    For Each sp In drw.SolidParts
        sp.Move dx, dy, dz
    Next sp
    On Error GoTo 0
End Sub

Private Sub ObrocWszystkieBrylyWokolX( _
    ByVal drw As Drawing, _
    ByVal sf As Object, _
    ByVal katStopnie As Double)

    On Error Resume Next
    If Not (sf Is Nothing) Then
        If sf.Bodies.Count > 0 Then
            Dim bItem As Object
            For Each bItem In sf.Bodies
                bItem.RotateG katStopnie, 0#, 0#, 0#, 1#, 0#, 0#
            Next bItem
            On Error GoTo 0
            Exit Sub
        End If
    End If

    Dim sp As Object
    For Each sp In drw.SolidParts
        sp.Selected = True
        drw.RotatePart katStopnie, 0#, 0#, 0#, 1#, 0#, 0#
        sp.Selected = False
    Next sp
    On Error GoTo 0
End Sub

Private Function CzyBrylaWymagaObrotuGoraDol( _
    ByVal sf As Object, _
    ByVal minX As Double, ByVal maxX As Double, _
    ByVal minY As Double, ByVal maxY As Double, _
    ByVal minZ As Double, ByVal maxZ As Double) As Boolean

    CzyBrylaWymagaObrotuGoraDol = False
    If sf Is Nothing Then Exit Function
    On Error Resume Next
    If sf.Bodies.Count = 0 Then Exit Function

    Dim lamelloGora As Long: lamelloGora = 0
    Dim lamelloDol As Long: lamelloDol = 0
    Dim lamelloBrzeg As Long: lamelloBrzeg = 0
    Dim otworFi6Gora As Long: otworFi6Gora = 0
    Dim otworFi6Dol As Long: otworFi6Dol = 0
    Dim otworySlepeGora As Long: otworySlepeGora = 0
    Dim otworySlepeDol As Long: otworySlepeDol = 0
    Dim petleGora As Long: petleGora = 0
    Dim petleDol As Long: petleDol = 0

    Dim midZPanel As Double
    midZPanel = (minZ + maxZ) / 2#

    Dim b As Object, f As Object
    For Each b In sf.Bodies
        For Each f In b.Faces
            Dim fType As Long
            fType = -1
            fType = CLng(f.Type)

            ' 1. Sprawdzenie powierzchni cylindrycznych:
            '    - gniazda Lamello P14 (gora, dol, brzeg: R ~ 50.2 mm)
            '    - otwory techniczne fi 6 pod klucz imbus (D ~ 6.0 mm, R ~ 3.0 mm) dla gniazd w brzegu
            '    - dowolne nieprzelotowe otwory pionowe (np. puszki fi 35, mimosrody fi 15, nawiercenia fi 3/5/8)
            If fType = 2 Then
                Dim c As Object
                Set c = f.Conic
                If Not (c Is Nothing) Then
                    If CBool(c.Hollow) Then
                        Dim rad As Double
                        rad = 0#
                        Err.Clear
                        rad = CDbl(c.Radius.Value)
                        If Err.Number <> 0 Or rad <= 0# Then
                            Err.Clear
                            rad = CDbl(c.Radius)
                        End If

                        Dim axDirX As Double, axDirY As Double, axDirZ As Double
                        axDirX = 0#: axDirY = 0#: axDirZ = 0#
                        Dim axObj As Object
                        Set axObj = Nothing
                        Set axObj = c.Axis
                        If Not (axObj Is Nothing) Then
                            Err.Clear
                            If Not (axObj.V Is Nothing) Then
                                axDirX = Abs(CDbl(axObj.V.dx))
                                axDirY = Abs(CDbl(axObj.V.dy))
                                axDirZ = Abs(CDbl(axObj.V.dz))
                                If Err.Number <> 0 Then
                                    Err.Clear
                                    axDirX = Abs(CDbl(axObj.V.dxDouble))
                                    axDirY = Abs(CDbl(axObj.V.dyDouble))
                                    axDirZ = Abs(CDbl(axObj.V.dzDouble))
                                End If
                            End If
                            If (axDirX + axDirY + axDirZ) < 0.0001 Then
                                Err.Clear
                                axDirX = Abs(CDbl(axObj.dx))
                                axDirY = Abs(CDbl(axObj.dy))
                                axDirZ = Abs(CDbl(axObj.dz))
                            End If
                        End If

                        Dim edg As Object
                        Dim hasC As Boolean: hasC = False
                        Dim cxMin As Double, cxMax As Double
                        Dim cyMin As Double, cyMax As Double
                        Dim czMin As Double, czMax As Double
                        For Each edg In f.Edges
                            If CLng(edg.Type) = 2 Then
                                If rad <= 0# Then
                                    Err.Clear
                                    rad = CDbl(edg.Radius.Value)
                                    If Err.Number <> 0 Or rad <= 0# Then
                                        Err.Clear
                                        rad = CDbl(edg.Radius)
                                    End If
                                End If
                                Dim cx As Double, cy As Double, cz As Double
                                If PobierzPunkt3D(edg.Center, cx, cy, cz) Then
                                    If Not hasC Then
                                        cxMin = cx: cxMax = cx
                                        cyMin = cy: cyMax = cy
                                        czMin = cz: czMax = cz
                                        hasC = True
                                    Else
                                        If cx < cxMin Then cxMin = cx
                                        If cx > cxMax Then cxMax = cx
                                        If cy < cyMin Then cyMin = cy
                                        If cy > cyMax Then cyMax = cy
                                        If cz < czMin Then czMin = cz
                                        If cz > czMax Then czMax = cz
                                    End If
                                End If
                            End If
                        Next edg

                        If Not hasC And Not (axObj Is Nothing) Then
                            Dim axPtC As Object
                            Set axPtC = Nothing
                            Set axPtC = axObj.P
                            If Not (axPtC Is Nothing) Then
                                Dim apxC As Double, apyC As Double, apzC As Double
                                If PobierzPunkt3D(axPtC, apxC, apyC, apzC) Then
                                    cxMin = apxC: cxMax = apxC
                                    cyMin = apyC: cyMax = apyC
                                    czMin = apzC: czMax = apzC
                                    hasC = True
                                End If
                            End If
                        End If

                        ' 1a. Tarcza Lamello P14 (R ~ 50.2 mm)
                        If rad >= LAMELLO_STEP_RAD_MIN And rad <= LAMELLO_STEP_RAD_MAX Then
                            If hasC Then
                                Dim cxMid As Double, cyMid As Double, czMid As Double
                                cxMid = (cxMin + cxMax) / 2#
                                cyMid = (cyMin + cyMax) / 2#
                                czMid = (czMin + czMax) / 2#
                                If czMid > (maxZ + 10#) Then
                                    lamelloGora = lamelloGora + 1
                                ElseIf czMid < (minZ - 10#) Then
                                    lamelloDol = lamelloDol + 1
                                ElseIf cxMid < (minX - 10#) Or cxMid > (maxX + 10#) Or _
                                       cyMid < (minY - 10#) Or cyMid > (maxY + 10#) Then
                                    lamelloBrzeg = lamelloBrzeg + 1
                                End If
                            Else
                                Dim vx1 As Double, vx2 As Double, vy1 As Double, vy2 As Double, vz1 As Double, vz2 As Double
                                If PobierzZakresWierzcholkowSciany(f, vx1, vx2, vy1, vy2, vz1, vz2) Then
                                    If (vz2 - vz1) <= 1.2 Then
                                        If Abs(vz2 - maxZ) <= 1.2 Then
                                            lamelloGora = lamelloGora + 1
                                        ElseIf Abs(vz1 - minZ) <= 1.2 Then
                                            lamelloDol = lamelloDol + 1
                                        End If
                                    ElseIf (vx2 - vx1) <= 1.2 Or (vy2 - vy1) <= 1.2 Then
                                        lamelloBrzeg = lamelloBrzeg + 1
                                    End If
                                End If
                            End If

                        ' 1b. Pionowe otwory nieprzelotowe (D <= 45 mm: w tym otwory fi 6 pod imbus, puszki fi 35, mimosrody fi 15, kolki fi 8, nawiercenia fi 3/5)
                        ElseIf rad > 0.25 And (rad * 2#) <= 45# Then
                            Dim czyOsPionowa As Boolean
                            czyOsPionowa = (axDirZ > 0.7 Or (axDirZ >= axDirX And axDirZ >= axDirY And axDirZ > 0.1))
                            If Not czyOsPionowa And hasC Then
                                If (cxMax - cxMin) <= 0.8 And (cyMax - cyMin) <= 0.8 And (czMax - czMin) > 0.5 Then
                                    czyOsPionowa = True
                                End If
                            End If

                            If czyOsPionowa Then
                                Dim hx1 As Double, hx2 As Double, hy1 As Double, hy2 As Double, hz1 As Double, hz2 As Double
                                Dim mamZakresZ As Boolean: mamZakresZ = False
                                If PobierzZakresWierzcholkowSciany(f, hx1, hx2, hy1, hy2, hz1, hz2) Then
                                    mamZakresZ = True
                                End If
                                If hasC And (czMax - czMin) > 0.5 Then
                                    If Not mamZakresZ Then
                                        hz1 = czMin: hz2 = czMax
                                        hx1 = cxMin: hx2 = cxMax
                                        hy1 = cyMin: hy2 = cyMax
                                        mamZakresZ = True
                                    Else
                                        If czMin < hz1 Then hz1 = czMin
                                        If czMax > hz2 Then hz2 = czMax
                                    End If
                                End If

                                If mamZakresZ Then
                                    ' Tylko otwory nieprzelotowe (slepe), ktore zaczynaja sie na jednej plaszczyznie i koncza wewnatrz plyty
                                    If (hz2 - hz1) > 0.5 And (hz2 - hz1) < ((maxZ - minZ) - 0.5) Then
                                        Dim hVertMidZ As Double
                                        hVertMidZ = (hz1 + hz2) / 2#
                                        Dim czyOdGory As Boolean, czyOdDolu As Boolean
                                        czyOdGory = (Abs(hz2 - maxZ) <= 0.8 And hz1 > (minZ + 0.5)) Or (hVertMidZ > (midZPanel + 0.5))
                                        czyOdDolu = (Abs(hz1 - minZ) <= 0.8 And hz2 < (maxZ - 0.5)) Or (hVertMidZ < (midZPanel - 0.5))

                                        If czyOdGory Then
                                            otworySlepeGora = otworySlepeGora + 1
                                        ElseIf czyOdDolu Then
                                            otworySlepeDol = otworySlepeDol + 1
                                        End If

                                        ' Dodatkowo dedykowany licznik dla otworow fi 6 mm pod klucz imbus przy krawedzi formatki
                                        If Abs((rad * 2#) - LAMELLO_P14_IMBUS_DIA) <= 0.6 Then
                                            Dim hMidX As Double, hMidY As Double
                                            If hasC Then
                                                hMidX = (cxMin + cxMax) / 2#
                                                hMidY = (cyMin + cyMax) / 2#
                                            Else
                                                hMidX = (hx1 + hx2) / 2#
                                                hMidY = (hy1 + hy2) / 2#
                                            End If
                                            If (hMidX - minX) <= 15# Or (maxX - hMidX) <= 15# Or _
                                               (hMidY - minY) <= 15# Or (maxY - hMidY) <= 15# Then
                                                If czyOdGory Then
                                                    otworFi6Gora = otworFi6Gora + 1
                                                ElseIf czyOdDolu Then
                                                    otworFi6Dol = otworFi6Dol + 1
                                                End If
                                            End If
                                        End If
                                    End If
                                End If
                            End If
                        End If
                    End If
                End If

            ' 2. Sprawdzenie petli wewnetrznych na gornej (maxZ) i dolnej (minZ) plaszczyznie formatki
            ElseIf fType = 1 Or CBool(f.IsPlanar) Then
                If f.Loops.Count > 1 Then
                    Dim px1 As Double, px2 As Double, py1 As Double, py2 As Double, pz1 As Double, pz2 As Double
                    If PobierzZakresWierzcholkowSciany(f, px1, px2, py1, py2, pz1, pz2) Then
                        If (pz2 - pz1) <= 0.8 And (px2 - px1) > 10# And (py2 - py1) > 10# Then
                            If Abs(pz2 - maxZ) <= 0.8 Then
                                petleGora = petleGora + (f.Loops.Count - 1)
                            ElseIf Abs(pz1 - minZ) <= 0.8 Then
                                petleDol = petleDol + (f.Loops.Count - 1)
                            End If
                        End If
                    End If
                End If
            End If
        Next f
    Next b

    ' Regula 1: Jesli formatka ma gniazda powierzchniowe Lamello P14 na dole (a na gorze brak),
    '           obracamy o 180 stopni, poniewaz tylko gorne wrzeciono (Face 5) pobiera frez z magazynku.
    If lamelloDol > 0 And lamelloGora = 0 Then
        CzyBrylaWymagaObrotuGoraDol = True

    ' Regula 2: Jesli formatka ma gniazda Lamello P14 w brzegu, a otwory techniczne fi 6 pod klucz imbus
    '           znajduja sie na dolnej powierzchni (minZ), obracamy formatke o 180 stopni, aby otwory fi 6 byly od gory (Face 5).
    ElseIf lamelloBrzeg > 0 And otworFi6Dol > 0 And otworFi6Gora = 0 Then
        CzyBrylaWymagaObrotuGoraDol = True

    ' Regula 3: Jesli nieprzelotowe otwory pionowe (np. puszki fi 35, mimosrody fi 15, nawiercenia fi 3/5/8)
    '           znajduja sie wylacznie na dolnej powierzchni (minZ), obracamy formatke o 180 stopni do gory (Face 5).
    ElseIf lamelloGora = 0 And otworySlepeDol > 0 And otworySlepeGora = 0 Then
        CzyBrylaWymagaObrotuGoraDol = True

    ' Regula 4: Jesli otwory fi 6 lub petle wewnetrzne sa wylacznie na dolnej powierzchni (a gora jest pusta),
    '           obracamy formatke o 180 stopni do gory.
    ElseIf otworFi6Dol > 0 And otworFi6Gora = 0 And petleGora = 0 Then
        CzyBrylaWymagaObrotuGoraDol = True
    ElseIf lamelloGora = 0 And lamelloDol = 0 And petleGora = 0 And petleDol > 0 Then
        CzyBrylaWymagaObrotuGoraDol = True
    End If
    On Error GoTo 0
End Function

Private Sub WyzerujKolorBrylyNaBialy(ByVal drw As Drawing, ByVal sf As Object)
    On Error Resume Next
    Dim bialyRGB As Long
    bialyRGB = RGB(255, 255, 255)

    If Not (sf Is Nothing) Then
        Dim b As Object
        Dim sciana As Object
        For Each b In sf.Bodies
            b.Color = bialyRGB
            If Not (b.Layer Is Nothing) Then
                b.Layer.Color = 7
                b.Layer.ColorRGB = bialyRGB
            End If
            For Each sciana In b.Faces
                sciana.Color = bialyRGB
            Next sciana
        Next b
    End If

    Dim sp As Object
    For Each sp In drw.SolidParts
        If Not (sp.GetLayer Is Nothing) Then
            sp.GetLayer.Color = 7
            sp.GetLayer.ColorRGB = bialyRGB
        End If
    Next sp
    On Error GoTo 0
End Sub

' ==============================================================================
' OKNO DIALOGOWE MENU HTA (STYL JAK W KONWERTERZE STEP -> DXF)
' ==============================================================================
Private Function PokazOknoMenuStepDoScx(ByRef outTrybAkcji As String) As Boolean
    PokazOknoMenuStepDoScx = False
    outTrybAkcji = ""

    On Error GoTo FallbackDialog

    Dim fso As Object
    Set fso = CreateObject("Scripting.FileSystemObject")

    Dim tempFolder As String
    tempFolder = Environ$("TEMP")
    If tempFolder = "" Then tempFolder = "C:\ALPHACAM\LICOMDIR\VBMacros"

    Dim htaFile As String
    Dim outFile As String
    htaFile = tempFolder & "\AcamStepDoScxMenu.hta"
    outFile = tempFolder & "\AcamStepDoScxMenu_Out.txt"

    If fso.FileExists(outFile) Then fso.DeleteFile outFile, True

    Dim outFileJS As String
    outFileJS = Replace(outFile, "\", "\\")

    Dim chkGzymki As String, chkSubFolder As String, chkWhite As String, chkTip As String, chkPartial As String
    If g_ZamienGzymki Then chkGzymki = "checked" Else chkGzymki = ""
    If g_ZapisujWPodfolderze Then chkSubFolder = "checked" Else chkSubFolder = ""
    If g_ZerujKolorBialy Then chkWhite = "checked" Else chkWhite = ""
    If g_KompensujStozekWiertla Then chkTip = "checked" Else chkTip = ""
    If g_UwzgledniajCzescioweOtwory Then chkPartial = "checked" Else chkPartial = ""

    Dim tsHta As Object
    Set tsHta = fso.CreateTextFile(htaFile, True, False)

    tsHta.WriteLine "<!DOCTYPE html>"
    tsHta.WriteLine "<html><head><meta http-equiv='X-UA-Compatible' content='IE=9'>"
    tsHta.WriteLine "<title>NANXING: STEP -> SCX</title>"
    tsHta.WriteLine "<hta:application id='oHTA' applicationname='AcamStepToScx' border='dialog' maximizebutton='no' minimizebutton='no' sysmenu='yes' scroll='no' singleinstance='yes' showintaskbar='yes' />"
    tsHta.WriteLine "<style>"
    tsHta.WriteLine "body { font-family: 'Segoe UI', Tahoma, sans-serif; font-size: 12px; background: #f3f4f6; margin: 12px 14px; color: #1f2937; }"
    tsHta.WriteLine ".header-bar { background: #1e3a8a; color: #ffffff; padding: 7px 12px; border-radius: 4px; font-size: 12px; font-weight: 700; margin-bottom: 8px; }"
    tsHta.WriteLine ".card { background: #ffffff; border: 1px solid #d1d5db; border-radius: 6px; padding: 10px 12px; margin-bottom: 10px; }"
    tsHta.WriteLine ".chk-row { margin: 8px 0; }"
    tsHta.WriteLine ".chk-row label { font-weight: normal; cursor: pointer; display: block; font-size: 12px; }"
    tsHta.WriteLine ".btns { margin-top: 10px; text-align: right; white-space: nowrap; }"
    tsHta.WriteLine "button { padding: 7px 12px; font-size: 12px; font-weight: 600; border-radius: 4px; cursor: pointer; margin-left: 6px; }"
    tsHta.WriteLine ".btn-single { background: #2563eb; color: #fff; border: 1px solid #1d4ed8; }"
    tsHta.WriteLine ".btn-all { background: #059669; color: #fff; border: 1px solid #047857; }"
    tsHta.WriteLine ".btn-cancel { background: #e5e7eb; color: #374151; border: 1px solid #9ca3af; }"
    tsHta.WriteLine "</style>"
    tsHta.WriteLine ""
    tsHta.WriteLine "<script language='JScript'>"
    tsHta.WriteLine "var outFile = '" & outFileJS & "';"
    tsHta.WriteLine "window.onload = function() {"
    tsHta.WriteLine "  var w = 460;"
    tsHta.WriteLine "  var h = 285;"
    tsHta.WriteLine "  window.resizeTo(w, h);"
    tsHta.WriteLine "  var reqH = document.body.scrollHeight + 55;"
    tsHta.WriteLine "  if (reqH > h) h = reqH;"
    tsHta.WriteLine "  window.resizeTo(w, h);"
    tsHta.WriteLine "  window.moveTo((screen.availWidth - w) / 2, (screen.availHeight - h) / 2);"
    tsHta.WriteLine "};"
    tsHta.WriteLine "function submitAction(mode) {"
    tsHta.WriteLine "  var gzymki = document.getElementById('chkGzymki').checked ? '1' : '0';"
    tsHta.WriteLine "  var subFolder = document.getElementById('chkSubFolder').checked ? '1' : '0';"
    tsHta.WriteLine "  var white = document.getElementById('chkWhite').checked ? '1' : '0';"
    tsHta.WriteLine "  var tip = document.getElementById('chkTip').checked ? '1' : '0';"
    tsHta.WriteLine "  var partial = document.getElementById('chkPartial').checked ? '1' : '0';"
    tsHta.WriteLine "  var fso = new ActiveXObject('Scripting.FileSystemObject');"
    tsHta.WriteLine "  var ts = fso.CreateTextFile(outFile, true);"
    tsHta.WriteLine "  ts.WriteLine(mode + '|' + gzymki + '|' + subFolder + '|' + white + '|' + tip + '|' + partial);"
    tsHta.WriteLine "  ts.Close();"
    tsHta.WriteLine "  window.close();"
    tsHta.WriteLine "}"
    tsHta.WriteLine "</script></head>"
    tsHta.WriteLine ""
    tsHta.WriteLine "<body onkeydown='if(event.keyCode===27) window.close();'>"
    tsHta.WriteLine "  <div class='header-bar'>Wiertarka NANXING: STEP -> SCX</div>"
    tsHta.WriteLine "  <div class='card'>"
    tsHta.WriteLine "    <div class='chk-row'><label><input type='checkbox' id='chkTip' " & chkTip & " /> Kompensacja stozka wiertla 118&deg;</label></div>"
    tsHta.WriteLine "    <div class='chk-row'><label><input type='checkbox' id='chkGzymki' " & chkGzymki & " /> Zamieniaj gzymki fi 11 na fi 12</label></div>"
    tsHta.WriteLine "    <div class='chk-row'><label><input type='checkbox' id='chkPartial' " & chkPartial & " /> Uwzgledniaj czesciowe otwory (IncludePartialHoles)</label></div>"
    tsHta.WriteLine "    <div class='chk-row'><label><input type='checkbox' id='chkSubFolder' " & chkSubFolder & " /> Zapisuj w podfolderze SCX_NANXING\</label></div>"
    tsHta.WriteLine "    <div class='chk-row'><label><input type='checkbox' id='chkWhite' " & chkWhite & " /> Zeruj kolor bryly na bialy</label></div>"
    tsHta.WriteLine "  </div>"
    tsHta.WriteLine "  <div class='btns'>"
    tsHta.WriteLine "    <button type='button' class='btn-cancel' onclick='window.close()'>Anuluj</button>"
    tsHta.WriteLine "    <button type='button' class='btn-all' onclick=""submitAction('FOLDER')"">Konwertuj FOLDER</button>"
    tsHta.WriteLine "    <button type='button' class='btn-single' onclick=""submitAction('SINGLE')"">Wybierz PLIK</button>"
    tsHta.WriteLine "  </div>"
    tsHta.WriteLine "</body></html>"
    tsHta.Close

    Dim wsh As Object
    Set wsh = CreateObject("WScript.Shell")
    wsh.Run "mshta.exe """ & htaFile & """", 1, True

    If fso.FileExists(htaFile) Then fso.DeleteFile htaFile, True
    If Not fso.FileExists(outFile) Then Exit Function

    Dim tsOut As Object
    Dim wynikLinia As String
    Set tsOut = fso.OpenTextFile(outFile, 1, False)
    If Not tsOut.AtEndOfStream Then
        wynikLinia = Trim$(tsOut.ReadLine)
    End If
    tsOut.Close
    fso.DeleteFile outFile, True

    If wynikLinia = "" Then Exit Function

    Dim czesci() As String
    czesci = Split(wynikLinia, "|")
    If UBound(czesci) >= 3 Then
        outTrybAkcji = czesci(0)
        g_ZamienGzymki = (czesci(1) = "1")
        g_ZapisujWPodfolderze = (czesci(2) = "1")
        g_ZerujKolorBialy = (czesci(3) = "1")
        If UBound(czesci) >= 4 Then
            g_KompensujStozekWiertla = (czesci(4) = "1")
        End If
        If UBound(czesci) >= 5 Then
            g_UwzgledniajCzescioweOtwory = (czesci(5) = "1")
        End If
        PokazOknoMenuStepDoScx = True
    End If
    Exit Function

FallbackDialog:
    Dim odp As VbMsgBoxResult
    odp = MsgBox("Wybierz tryb pracy dla wiertarki Nanxing:" & vbCrLf & vbCrLf & _
                 "[Tak] = Wybierz pojedynczy plik STEP" & vbCrLf & _
                 "[Nie] = Konwertuj caly folder STEP", _
                 vbYesNoCancel + vbQuestion, "STEP -> SCX (Nanxing)")
    If odp = vbYes Then
        outTrybAkcji = "SINGLE"
        PokazOknoMenuStepDoScx = True
    ElseIf odp = vbNo Then
        outTrybAkcji = "FOLDER"
        PokazOknoMenuStepDoScx = True
    End If
End Function

' ==============================================================================
' DIALOG WYBORU PLIKU WINDOWS (ZAPAMIETUJE OSTATNI FOLDER W SESJI)
' ==============================================================================
Private Function WybierzPlikStep_Dialog( _
    ByVal domyslnaSciezka As String, _
    Optional ByVal tytulOkna As String = "Wybierz plik .STEP / .STP (Nanxing)") As String

    WybierzPlikStep_Dialog = ""
    On Error GoTo FallbackInput

    Dim fso As Object
    Set fso = CreateObject("Scripting.FileSystemObject")

    Dim folderStart As String
    folderStart = Trim$(domyslnaSciezka)
    If folderStart <> "" Then
        If fso.FileExists(folderStart) Then
            folderStart = fso.GetParentFolderName(folderStart)
        ElseIf Not fso.FolderExists(folderStart) Then
            folderStart = ""
        End If
    End If

    If folderStart = "" Then
        folderStart = PobierzUniwersalnyFolderStartowy()
    End If

    Dim ofn As OPENFILENAMEW
    Dim filtr As String
    Dim plikBuf As String
    Dim tytulBuf As String
    Dim katalogPoczatkowy As String
    Dim naglowekOkna As String
    Dim defExt As String

    filtr = "Pliki STEP (*.stp;*.step)" & vbNullChar & "*.stp;*.step" & vbNullChar & _
            "Wszystkie pliki (*.*)" & vbNullChar & "*.*" & vbNullChar & vbNullChar

    plikBuf = String$(2048, vbNullChar)
    tytulBuf = String$(260, vbNullChar)
    katalogPoczatkowy = folderStart & vbNullChar
    naglowekOkna = tytulOkna & vbNullChar
    defExt = "step" & vbNullChar

    ofn.lStructSize = LenB(ofn)
    ofn.hwndOwner = 0
    ofn.lpstrFilter = StrPtr(filtr)
    ofn.nFilterIndex = 1
    ofn.lpstrFile = StrPtr(plikBuf)
    ofn.nMaxFile = Len(plikBuf)
    ofn.lpstrFileTitle = StrPtr(tytulBuf)
    ofn.nMaxFileTitle = Len(tytulBuf)
    ofn.lpstrInitialDir = StrPtr(katalogPoczatkowy)
    ofn.lpstrTitle = StrPtr(naglowekOkna)
    ofn.flags = &H8180C& ' OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_HIDEREADONLY
    ofn.lpstrDefExt = StrPtr(defExt)

    Dim res As Long
    res = GetOpenFileNameW(ofn)
    If res <> 0 Then
        Dim pozZero As Long
        pozZero = InStr(1, plikBuf, vbNullChar)
        If pozZero > 1 Then
            WybierzPlikStep_Dialog = Left$(plikBuf, pozZero - 1)
        Else
            WybierzPlikStep_Dialog = Trim$(plikBuf)
        End If
    End If
    Exit Function

FallbackInput:
    WybierzPlikStep_Dialog = InputBox("Podaj pelna sciezke do pliku .stp / .step:", _
                                      tytulOkna, PobierzUniwersalnyFolderStartowy())
End Function
