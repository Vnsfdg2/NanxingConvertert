# Baza wiedzy: NanxingConvertert (STEP → SCX)

Skondensowana dokumentacja domenowa, technologiczna i UI przeniesiona z projektu prototypowego (`ALKATRAS_BSOLID/ALPHACAM/NANXING`) na potrzeby budowy samodzielnej aplikacji desktopowej na Windows (docelowo **Rust**).

---

## 1. Cel aplikacji
Samodzielny konwerter i edytor orientacji formatek (`STEP` → `.scx`) dla **6-stronnej wiertarki CNC Nanxing**, eliminujący pośrednie makro w AlphaCAM:
- bezpośredni odczyt geometrii 3D B-Rep z plików `STEP` (`ISO-10303-21`),
- automatyczne rozpoznanie wierceń, gniazd Lamello P14, nutowania i frezowań,
- podgląd 2D / X-Ray formatki wraz z mocowaniem maszynowym,
- ręczne korygowanie orientacji na maszynie (`90°`, `180°`, obrót Góra/Dół, lustro `X/Y`) przed wygenerowaniem `.scx`,
- nakładanie obrzeży na podstawie pliku `.csv` z programu do rozkroju,
- eksport czystych plików XML `.scx` (z pominięciem pustych formatek bez obróbek).

---

## 2. Maszyna: 6-stronna wiertarka Nanxing

### Układ współrzędnych i ściany (`Face`)
- Wymiary formatki: `Length` (oś `X`), `Width` (oś `Y`), `Thickness` (`T`, oś `Z` liczona od spodu `0` do góry `T`).
- `Face="5"` — Góra (`Z = T`)
- `Face="6"` — Dół (`Z = 0`)
- `Face="1"` — Tył (`Y = Width`)
- `Face="2"` — Przód (`Y = 0`)
- `Face="3"` — Prawo (`X = Length`)
- `Face="4"` — Lewo (`X = 0`)

### Mocowanie i pozycjonowanie (widok bazowy)
- **Dwie prostokątne łapy zaciskowe** u góry formatki (rozstaw dopasowuje się do bieżącej szerokości/orientacji formatki).
- **Okrągły bolec pozycjonujący** w prawym górnym narożniku.

### Agregaty i wrzeciona
1. **Głowica wiertarska 6-stronna**: wiercenia pionowe (`Face 5, 6`) i poziome (`Face 1..4`).
2. **Agregat do nutowania (piła)**:
   - tnie **wyłącznie w osi `X`**,
   - fizyczna tarcza ma grubość **`4 mm`** (domyślna szerokość rowka w programie: **`6 mm`**),
   - wybór odsunięcia / strony względem linii tarczy (`ToolOffset="中"` = środek; przy szerokości większej niż `4 mm` maszyna wykonuje przejścia po obu stronach).
3. **Górne wrzeciono z wymianą narzędzi (ATC, `Face="5"`)**:
   - kanały pod taśmy LED,
   - duże otwory przelotowe w blacie (np. `Ø60` na przewody),
   - wycięcia brzegowe i ścięcia narożników,
   - frezowanie gniazd powierzchniowych Lamello P14 (`Face="5"`).
4. **Dolne stałe wrzeciono (`Face="6"`)**:
   - zamontowany na stałe frez **`Ø12 mm`**,
   - otwarte kontury / gniazda wychodzące w krawędź od spodu formatki.
5. **Tarcza boczna Lamello**: gniazda Lamello P14 w 4 krawędziach (`Face 1..4`).

---

## 3. Specyfikacja formatu XML `.scx`

Zweryfikowana na pliku `program testowy.scx` oraz działającym eksporterze `StepDoSCX_Nanxing.bas`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<Root Cad="BuiltInCad" version="2.0">
  <Project>
    <Panels>
      <Panel IsProduce="true" ID="nazwa" Name="nazwa" Length="400" Width="300" Thickness="18" MachiningPoint="1">
        <Outline>
          <Point X="400" Y="300" />
          <Point X="0" Y="300" />
          <Point X="0" Y="0" />
          <Point X="400" Y="0" />
        </Outline>
        <Innerlines />
        <Machines>
          <!-- Operacje Type="1", "2", "3", "4", "1002" -->
        </Machines>
        <EdgeGroup X1="0" Y1="0">
          <Edge Face="2" Thickness="0" Pre_Milling="0" X="0" Y="0" CentralAngle="0" />
          <Edge Face="1" Thickness="0" Pre_Milling="0" X="0" Y="0" CentralAngle="0" />
          <Edge Face="4" Thickness="0" Pre_Milling="0" X="0" Y="0" CentralAngle="0" />
          <Edge Face="3" Thickness="0" Pre_Milling="0" X="0" Y="0" CentralAngle="0" />
        </EdgeGroup>
      </Panel>
    </Panels>
  </Project>
</Root>
```

### Typy operacji `<Machining>`
- **Wiercenie poziome (`Type="1"`)**:
  ```xml
  <Machining Type="1" IsGenCode="2" Face="1..4" X="..." Y="..." Z="..." Diameter="..." Depth="..." />
  ```
- **Wiercenie pionowe (`Type="2"`)**:
  ```xml
  <Machining Type="2" IsGenCode="2" Face="5|6" X="..." Y="..." Diameter="..." Depth="..." />
  ```
- **Frezowanie (`Type="3"`)**:
  ```xml
  <Machining Type="3" IsGenCode="2" Face="5|6" X="..." Y="..." Depth="10" Diameter="0" ToolOffset="Undefined" DeepOffset="0" Pocket="1|0" Closed="1|0">
    <MillPreDefinedRectangle Length="0" Width="0" X="0" Y="0" Z="0" Radius="0" CornerBotLeft="0" CornerBotRight="0" CornerTopLeft="0" CornerTopRight="0" />
    <Lines>
      <Line LineID="0" EndX="..." EndY="..." Angle="0" />
    </Lines>
  </Machining>
  ```
  - Kieszeń / wycięcie zamknięte: `Pocket="1" Closed="1"`.
  - Wycięcie na wylot płyty `18 mm`: `Depth="18.1"`.
  - Okrąg: 4 odcinki `<Line>` z `Angle="-90"`.
  - Otwarty kontur (np. dolne wrzeciono `Ø12` wychodzące w krawędź): `Face="6" Pocket="0" Closed="0"`.
- **Nutowanie piłą w osi `X` (`Type="4"`)**:
  ```xml
  <Machining Type="4" IsGenCode="2" Face="5" X="400" Y="264.919" Z="18" EndX="0" EndY="264.919" EndZ="18" Depth="5" Width="6" ToolOffset="中" />
  ```
  *(Uwaga: atrybut `ToolOffset` wykorzystuje chiński znak `中` dla środka).*
- **Lamello P14 (`Type="1002"`)**:
  ```xml
  <Machining Type="1002" IsGenCode="2" Face="1..5" Direction="X|Y" X="..." Y="..." Z="..." Length="66" Width="9.8|10.15" MaxDepth="14.1" />
  ```
  - Powierzchnia górna (`Face="5"`): `Width="9.8"` (frez).
  - Ściany boczne (`Face="1..4"`): `Width="10.15"` (tarcza), `Direction="X"` dla `Face 1,2`, `Direction="Y"` dla `Face 3,4`.

### Obrzeża `<EdgeGroup>` (zweryfikowane na `NangXinPCDrillCAM` + `EDGE_NANXING.scx`)
- **Mapowanie krawędzi w `<EdgeGroup>`** *(uwaga: inne niż w `StepDoSCX_Nanxing.bas` dla wierceń!)*:
  - `Face="1"` = **Dół** (`Y = 0`, przy osi `X`)
  - `Face="2"` = **Góra** (`Y = Width`, od strony łap zaciskowych)
  - `Face="3"` = **Lewo** (`X = 0`, przy punkcie `(0,0)`)
  - `Face="4"` = **Prawo** (`X = Length`, od strony bolca pozycjonującego)
- **Zasada wyświetlania (pełny zielony vs ramka)**:
  - `Thickness="0"` → brak obrzeża (brak znacznika).
  - Dla niezerowych krawędzi (`Thickness > 0`) program liczy średnią ich grubości na formatce: krawędzie `> średnia` rysuje jako **pełny zielony prostokąt**, a `<= średnia` jako **zieloną ramkę**.
  - Atrybuty `Pre_Milling`, `X`, `Y`, `CentralAngle` oraz `EdgeGroup X1/Y1` nie wpływają na wyświetlanie ani pozycjonowanie obrzeży.

### Identyfikacja formatki i zarys `<Outline>` (zweryfikowane na `NangXinPCDrillCAM`)
- **`ID` (kod kreskowy / QR)**: wyświetlany w kolumnie `ID` oraz używany przez skaner kodów wiertarki. Docelowo **krótki, prosty kod** wycinany z nazwy pliku (np. `S1_03`, `S5_30` — numer szafki + globalny numer formatki w zamówieniu).
- **`Name` (kolumna `Info.`)**: wyświetlany w kolumnie `Info.` tabeli programów — tu trafia **pełna nazwa formatki 1:1** zgodna ze STEP / DXF / rysunkiem technicznym.
- **`Material` (kolumna `Materiał`)**: wyświetlany bezpośrednio w kolumnie `Materiał` w tabeli programów.
- **`Grain` (`L` / `W`)**: akceptowany w XML, ale w obecnej wersji `NangXinPCDrillCAM` nie ma widocznego znacznika graficznego.
- **`<Outline>` (realny kontur formatki)**:
  - Zarówno `<Outline>`, jak i `<Machining>` przy `MachiningPoint="1"` mają ten sam układ odniesienia: w widoku 2D wiertarki punkt `(0, 0)` z XML ląduje w **prawym górnym rogu** przy bolcu pozycjonującym (`X_widok = Length - X_xml`, `Y_widok = Width - Y_xml`).
  - Wiertarka rysuje dokładny wielokąt z `<Point X="..." Y="..." />` — skosy, podcięcia cokołowe i kształty `L` są od razu widoczne na podglądzie operatora (a na kształcie `L` widać też, czy łapy zaciskowe nie wiszą w powietrzu).
  - Prawidłowy atrybut dla łuku narożnika w `<Outline>` to **`Radius2="..."`** (np. `Radius2="60"`). `Radius` i `Angle` rysują proste cięciwy.
  - **`<Innerlines>`** nie jest obsługiwane przez parser; wycięcie wewnętrzne można zwizualizować w `<Outline>` przez szczelinę `0.01 mm` od krawędzi do otworu.
  - **Zakaz używania `IsGenCode="0"` jako znacznika wizualnego**: pojawia się w tabeli `Feature` jako wyłączona operacja i grozi przypadkowym zaznaczeniem („zaznacz wszystko”) oraz wyfrezowaniem na maszynie.

### Pełna mapa konfiguracji `SyntecCad.config` (`NangXinPCDrillCAM`)

| Sekcja | Klucz w `SyntecCad.config` | Wartość / ścieżka w `.scx` | Znaczenie |
| :--- | :--- | :--- | :--- |
| **Plik** | `FileType` / `FileExtensionName` | `xml` (`xml`, `scx`) | Format parsera XML |
| **Ściany** | `Face1..Face6`, `WorkingFace` | `1..6`, atrybut `Face` | Identyfikacja 6 ścian formatki |
| **Punkt bazowy** | `AnchorPoint` | `MachiningPoint` | Punkt bazowy na formatce (`1`) |
| **Flagi** | `Empty` / `EmptyEnable` / `EmptyDisable` | `Pocket` (`1` / `0`) | Kieszeń / wycięcie zamknięte |
| **Flagi** | `Close` / `CloseEnable` / `CloseDisable` | `Closed` (`1` / `0`) | Kontur zamknięty vs otwarty |
| **Flagi** | `Produce` / `ProduceEnable` / `ProduceDisable` | `IsGenCode` (`2` / `0`) | Czy generować kod NC dla operacji |
| **Kompensacja** | `ToolOffset` (`Left` / `Right` / `Middle`) | `ToolOffset` (`左` / `右` / `中`) | Strona narzędzia (lewo / prawo / środek) |
| **Narzędzia** | `ToolData` / `SawBlade` / `MainAxis` / `Drill` | `ToolName` (`jp` / `xxd` / `bbcd`) | Opcjonalne wskazanie narzędzia po nazwie |
| **Formatka** | `PanelPath` | `Root/Project/Panels/Panel` | Ścieżka do elementu płyty |
| **Formatka** | `PanelID` / `PanelName` | `ID` / `Name` | Identyfikator i nazwa formatki |
| **Formatka** | `PanelLength` / `PanelWidth` / `PanelThickness` | `Length` / `Width` / `Thickness` | Wymiary płyty (`X`, `Y`, `Z`) |
| **Formatka** | `PanelGrain` (`Length` / `Width`) | `Grain` (`L` / `W`) | Kierunek usłojenia |
| **Formatka** | `PanelMaterial` | `Material` | Nazwa materiału |
| **Kontur** | `PanelOutlinePath` / `X` / `Y` / `Bulge` | `Outline/Point` (`X`, `Y`, `Radius\|Radius2`) | Zarys zewnętrzny formatki |
| **Obrzeża** | `PanelFace1..4EdgeBandingPath`, `PanelEdgeThickness` | `EdgeGroup/Edge%Face=1..4%`, `Thickness` | Grubość obrzeży na 4 krawędziach |
| **Wiercenia** | `HoleVerticalPath` / `HoleHorizontalPath` | `Machines/Machining%Type=2%` / `Type=1` | Wiercenia pionowe / poziome |
| **Wiercenia** | `HoleCoordX/Y/Z`, `HoleSize`, `HoleDepth` | `X`, `Y`, `Z`, `Diameter`, `Depth` | Pozycja, średnica i głębokość |
| **Nutowanie** | `GroovePath`, `SideGroovePath` | `Machines/Machining%Type=4%` | Piła (góra i bok) |
| **Nutowanie** | `GrooveStartX/Y`, `EndX/Y`, `Width`, `Depth` (+ `Z`) | `X`, `Y`, `EndX`, `EndY`, `Width`, `Depth` | Geometria nuta |
| **Frezowanie** | `MillPath`, `MillCoordPath` | `Machines/Machining%Type=3%`, `.../Lines/Line` | Wrzeciono i ścieżka punktów |
| **Frezowanie** | `MillStartX/Y/Z`, `MillWidth`, `MillDepth` | `X`, `Y`, `Z`, `Diameter`, `Depth` | Punkt startowy, szerokość, głębokość |
| **Frezowanie** | `MillCoordX/Y/Z`, `MillAngleType`, `MillAngle`, `Is3DMill` | `EndX`, `EndY`, `EndZ`, `Degree`, `Angle%Reverse%`, `False` | Punkty konturu i łuki |

---

## 4. Sprawdzone reguły technologiczne (z produkcji)

1. **Analiza 3D B-Rep zamiast prostego rzutu 2D**:
   - Głębokość wierceń mierzyć bezpośrednio od powierzchni wejścia do **rzeczywistego dna otworu** w modelu 3D.
2. **Filtrowanie niepełnych otworów (`< 360°`)**:
   - Domyślnie odrzucać częściowe łuki (`IncludePartialHoles = false`), aby wewnętrzne promienie zaokrągleń (np. `R=12` w podcięciach pod cokół) nie tworzyły fałszywych wierceń.
3. **Otwory poziome (`Face 1..4`)**:
   - Bezwzględny limit głębokości: **`50 mm`** (maksymalna długość robocza wiertła bocznego).
   - Jeśli otwór poziomy **nie ma płaskiego dna** (np. otwór pod trzpień mimośrodu `Ø8` wpadający w pionowy cylinder `Ø15`), dodać **`+2 mm`** naddatku dla pełnego przebicia.
4. **Kompensacja stożka wiertła `118°`**:
   - Dla nieprzelotowych wierteł krętych `D <= 12.5 mm` doliczać stożek `(D / 2) / tan(59°)`: np. `Ø5 -> +1.5022 mm`, `Ø6 -> +1.8026 mm`, `Ø8 -> +2.4034 mm`.
   - Otwory płaskodenne `D >= 15 mm` (puszki `Ø35`, mimośrody `Ø15`) — **bez naddatku**.
5. **Automatyczny obrót formatki góra/dół (`180°` wokół osi `X`)**:
   - Gdy nieprzelotowe wiercenia pionowe (`Ø35`, `Ø15`, `Ø3/5/8`) znajdują się wyłącznie na spodzie (`Face 6`), obrócić formatkę, aby były na górze (`Face 5`).
   - Gdy powierzchnia ma gniazda Lamello P14 od spodu — obrócić na `Face 5` (tylko górne wrzeciono pobiera frez).
   - Gdy boczne Lamello P14 ma otwór `Ø6` pod klucz imbus od spodu — obrócić tak, aby `Ø6` był na `Face 5`.
6. **Lamello P14 — detekcja i otwór pod imbus `Ø6`**:
   - Detekcja z B-Rep: łuk o `R ≈ 50.2 mm` (`Ø ≈ 100.4 mm`), cięciwa `≈ 69.56 mm`, szerokość `≈ 10 mm`, głębokość `≈ 14 mm` (uwzględniać także cylindry dzielone szwem).
   - Dla każdego gniazda bocznego (`Face 1..4`) wygenerować na `Face 5` otwór `Ø6` w osi gniazda, **`7.5 mm`** od krawędzi formatki, o głębokości `Depth = T - Z` (bez stożka wiertła; płytki odcinek z STEP zastąpić pełną głębokością do środka gniazda).
7. **Opcjonalna zamiana gzymek**: `Ø11` → pogłębienie proste `Ø12`.
8. **Puste formatki**: nie tworzyć pliku `.scx`, jeśli formatka nie ma żadnych operacji (maszyna ma parametr `4957=1`, więc inaczej wygenerowałaby pusty program NC).
9. **Limity i ostrzeżenia maszyny (z `CustomParamValue.xml` / `Ustawienia - parametry`)**:
   - Minimalna dopuszczalna szerokość formatki (`90011`): **`45 mm`** (poniżej — blokada/błąd w konwerterze).
   - Ostrzeżenie przed przytrzaśnięciem przy łapach (`90056`): szerokość **`< 100 mm`** (oznaczyć ostrzeżeniem na liście w UI).
   - Walidacja łap zaciskowych: przy kształtach `L` i podcięciach cokołowych sprawdzać, czy obie górne łapy mają pod sobą pełny materiał (nie wiszą w wycięciu).
   - Domyślny kierunek obrotu góra/dół na maszynie: **`Odwróć według osi Y`**.
   - Aktywne parametry CNC w konfiguracji maszyny: `4930=1` (dobór frezu po geometrii, bez `ToolName`), `4955=1` (wykonywanie otworów frezem przy braku wiertła), `4991=1` (frezowanie zewnętrzne przeciwbieżne pod okleinę), `5271=1` (dodatkowe frezowanie przy dojściu do krawędzi).

---

## 5. Wytyczne UI / UX (Makieta)

- **Ciemny styl przemysłowy** (stonowany, wzorowany na zaawansowanych narzędziach CAD/CAM):
  - Lewy panel: lista załadowanych formatek z oznaczeniem liczby operacji, statusu (np. czy ma obrzeże/czy wymaga obrotu) i szybkim filtrem.
  - Środkowy viewport CAD: płynny zoom/pan, siatka, linie wymiarowe `L`/`W`, układ `(0,0)`, tryby `X-Ray / Góra (F5) / Dół (F6)`, przyciski orientacji (`90°`, `180°`, `Góra/Dół`, `Lustro X`, `Lustro Y`), dynamiczne **2 łapy mocujące + bolec pozycjonujący**, interaktywne tooltipy oraz synchronizacja zaznaczenia z tabelą operacji.
  - Prawy inspektor (3 zakładki): `Właściwości` (w tym klikalny diagram 4 krawędzi pod obrzeża CSV i przełączniki reguł), `Operacje` (lista/tabela), `SCX XML` (podgląd generowanego kodu na żywo).

---

## 6. Pliki referencyjne i brakujące próbki

### Dostępne w poprzednim workspace (`D:\WorkerResource\ALKATRAS_BSOLID`):
- `ALPHACAM\NANXING\makieta_nanxing_studio.html` — zatwierdzona makieta UI.
- `ALPHACAM\NANXING\StepDoSCX_Nanxing.bas` — pełny działający algorytm B-Rep, Lamello P14 i generator `.scx`.
- `ALPHACAM\NANXING\README.md` — opis reguł dodatku AlphaCAM.
- `program testowy.scx` — wzorcowy plik XML z operacjami `Type="3"` i `Type="4"`.

### Do uzupełnienia w trakcie prac nad `NanxingConvertert`:
- [x] Przykładowy plik `.scx` z zapisanymi realnymi parametrami obrzeży w `<EdgeGroup>` (`EDGE_NANXING.scx` + testy w `NangXinPCDrillCAM`).
- [ ] Przykładowy plik `.csv` z programu do rozkroju (mapowanie kolumn i krawędzi okleinowania).
