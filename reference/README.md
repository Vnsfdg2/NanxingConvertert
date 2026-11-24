# Dodatek AlphaCAM: Konwerter STEP -> SCX (Nanxing 6-stronna wiertarka)

Katalog zawiera w pełni odseparowany pakiet dodatku do AlphaCAM, niezależny od pakietu bSolid.

## Zawartość folderu:
1. **`Events.bas`** – rejestruje dedykowaną kategorię w menu Dodatków:
   * `Dodatki -> NANXING -> STEP -> SCX (Nanxing)`
2. **`StepDoSCX_Nanxing.bas`** – główny silnik konwersji modeli 3D STEP do programów XML `.scx`:
   * Wiercenie pionowe: Góra (`Face 5`), Dół (`Face 6`)
   * Wiercenie poziome: Tył (`Face 1`), Przód (`Face 2`), Prawo (`Face 3`), Lewo (`Face 4`) z dynamicznym odczytem osi $Z$ (wysokość od spodu formatki)
   * **Obsługa gniazd Lamello P14 (`Type="1002"`)**:
     * Automatyczne filtrowanie z modelu 3D STEP wycięć pod złącza Lamello P14 ($R \approx 50.2$ mm / $\varnothing \approx 100.4$ mm, cięciwa $\approx 69.56$ mm, szerokość $\approx 10$ mm, głębokość $\approx 14$ mm) z marginesem błędu (obsługuje również cylindry dzielone szwem z generatora SolidCraft)
     * Automatyczne obracanie formatki góra/dół ($180^\circ$ wokół osi X):
       * dla formatek z gniazdami powierzchniowymi Lamello P14 – tak, aby gniazda zawsze znalazły się od góry (`Face 5`, $Z = T$), ponieważ tylko górne wrzeciono pobiera frez z magazynku,
       * dla formatek z gniazdami Lamello P14 w brzegu płyty – wykrywanie wewnętrznego otworu $\varnothing 6$ mm pod klucz imbus i automatyczne obracanie formatki tak, aby strona z otworem $\varnothing 6$ mm była skierowana do góry (`Face 5`)
     * Gniazda w powierzchni górnej (`Face 5`, $Z = T$) wykonywane frezem: stałe `Length="66" Width="9.8" MaxDepth="14.1"`, dynamiczny kierunek (`Direction="X"` lub `"Y"`) oraz środek gniazda `(X, Y)` wyliczany z modelu STEP
     * Gniazda w 4 ścianach bocznych (`Face 1, 2, 3, 4`) wykonywane tarczą: stałe `Length="66" Width="10.15" MaxDepth="14.1"`, dynamiczne rozpoznanie ściany (`Face 1..4`), kierunku (`Direction="X"` dla `Face 1, 2` oraz `Direction="Y"` dla `Face 3, 4`) i położenia środka gniazda w osiach `X/Y` oraz `Z`
     * Automatyczne dopisywanie otworu technicznego $\varnothing 6$ pod klucz imbus na `Face 5` dla każdego gniazda Lamello P14 w brzegu płyty: idealnie na środku gniazda, $7.5$ mm od brzegu gniazda formatki, o głębokości do środka wysokości gniazda (`Depth = T - Z`, bez doliczania stożka wiertła; płytki odcinek $\varnothing 6$ z modelu STEP przed kieszenią Lamello jest zastępowany pełną głębokością do środka gniazda)
   *Automatyczna kompensacja stożka wiertła 118° dla otworów nieprzelotowych wiertłami krętymi ($D \le 12.5$ mm): np. $\varnothing 5 \rightarrow +1.5022$ mm (13 mm w STEP $\rightarrow$ 14.5022 mm w SCX), $\varnothing 6 \rightarrow +1.8026$ mm, $\varnothing 8 \rightarrow +2.4034$ mm (otwory płaskodenne $\ge 15$ mm bez naddatku)
   * **Weryfikacja geometrii B-Rep 3D (`WeryfikujOtworZBRep`)**:
     * Opcja w GUI **`Uwzgledniaj czesciowe otwory (IncludePartialHoles)`** (domyślnie **`FALSE`**): odrzuca niepełne cylindry $< 360^\circ$, np. wewnętrzne promienie zaokrągleń w narożnikach podcięć pod listwę przypodłogową
     * Pomiar głębokości otworów pionowych i poziomych bezpośrednio od powierzchni wejścia do **dna otworu** w modelu 3D (wraz z automatycznym obrotem formatki o $180^\circ$, jeśli nieprzelotowe otwory pionowe — puszki $\varnothing 35$, mimośrody $\varnothing 15$, nawiercenia $\varnothing 3/5/8$ — znalazły się na spodzie)
     * Automatyczny naddatek **`+2 mm`** dla otworów poziomych bez płaskiego dna (np. otwór poziomy pod trzpień mimośrodu wpadający bezpośrednio w cylinder otworu pionowego $\varnothing 15$), gwarantujący pełne przebicie
     * Bezwzględny limit maksymalnej głębokości wiercenia bocznego (`Face 1..4`) do **`50 mm`** (maksymalna długość robocza wiertła)
     * Automatyczne pomijanie generowania plików `.scx` dla gładkich formatek bez żadnych operacji wiercenia i bez gniazd Lamello
   * Okno HTA z wyborem pojedynczego pliku lub całego folderu
   * Opcjonalna zamiana gzymek $\varnothing 11$ na pogłębienie proste $\varnothing 12$
   * Zapis do podfolderu `SCX_NANXING\`

## Jak załadować dodatek w AlphaCAM jako osobny projekt:
1. W AlphaCAM naciśnij `Alt + F11` (edytor VBA).
2. Wybierz `File -> Import File...` i wskaż z tego folderu:
   * `Events.bas`
   * `StepDoSCX_Nanxing.bas`
3. Zapisz projekt jako plik binary macro `NANXING.arb` w folderze:
   `C:\ALPHACAM\LICOMDIR\VBMacros\Startup\NANXING.arb`
   *(dzięki temu AlphaCAM przy każdym uruchomieniu automatycznie załaduje oba dodatki: BSOLID.arb oraz NANXING.arb)*.
