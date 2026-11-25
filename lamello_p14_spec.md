# Lamello P14 (Clamex / Tenso) — specyfikacja grafiki dla implementacji w Rust

Kod referencyjny: `lamello_p14.rs` (czysty `std`, 6 testów przechodzi). Pierwowzór wizualny: `nanxing-makieta.html`
(funkcje `sockOutline`, gałęzie `'lamello'` / `'lamsock'` w `buildOps`, `opSVG`, `edgePoint`).

## 1. Dwie grafiki, jedna tarcza

Obie nuty powstają tą samą tarczą **Ø100,4 mm × 7 mm**. Różnią się głębokością i widokiem.

| Element | Widok na podglądzie | Kształt | Głębokość | Długość w widoku |
|---|---|---|---|---|
| Nuta w krawędzi | z góry, na krawędzi formatki | **odcinek koła** ucięty cięciwą | 14 mm | cięciwa **69,56 mm** |
| Gniazdo w powierzchni | z góry, na licu formatki | **„kość”**: wąski środek + pogrubione końce | 10 mm | cięciwa **60,13 mm** |
| Otwór pod klucz (tylko Clamex) | z góry | koło Ø5–6 + kropka środka | ok. 13 mm (założenie) | — |

Wzór na cięciwę: `chord = 2·√(R² − (R − h)²)`, R = 50,2.

## 2. Nuta w krawędzi — odcinek koła

```
 krawędź formatki (d = 0)   ─────────[ cięciwa 69,56 ]─────────
                                \                             /
                                 \        ○ Ø6 (7,5 od kr.)  /     ← tylko Clamex
                                  \_________________________/
                                              ↑ d = 14 (najgłębszy punkt, s = s0)
 Środek tarczy: (s0, 14 − 50,2) = poza formatką.
```

Algorytm (`edge_slot_local`): θ = acos((R−h)/R); dla k = 0..20: a = −θ + 2θ·k/20;
punkt = `(s0 + R·sin a,  h − R + R·cos a)` w układzie (s, d). Wielokąt jest zamknięty niejawnie cięciwą.
Potem `edge_point(F, L, W, s, d)` przenosi punkt do układu formatki:

| Krawędź | (u, v) |
|---|---|
| F1 (dół) | (s, d) |
| F2 (prawa) | (L − d, s) |
| F3 (góra) | (s, W − d) |
| F4 (lewa) | (d, s) |

## 3. Gniazdo w powierzchni — „kość”

```
        ←────────── 60,13 (cięciwa przy 10 mm) ──────────→
        ╭──╮                                          ╭──╮
       (    )══════════════════════════════════════(    )     szerokość końców 10
        ╰──╯        środek 7 mm szerokości           ╰──╯
       ←7→                                           ←7→      długość końców 7, R = 5 = połowa 10 → pełne półkole
```

Parametry: `sock_w = 7`, `end_len = 7`, `end_w = 10`, `end_r = 5`.
Kontur (34 punkty, `face_socket_local`): środek (a, b) = (0, 0), a wzdłuż osi:
`(-h+el,-nw) → (h-el,-nw) → (h-el,-e2) → (h-r,-e2) → łuk → łuk → (h-el,e2) → (h-el,nw) → (-h+el,nw) → (-h+el,e2) → (-h+r,e2) → łuk → łuk → (-h+el,-e2)`
gdzie `h = chord/2`, `nw = 3,5`, `e2 = 5`, `el = 7`. Łuk = 6 punktów (5 odcinków) na ćwierćokręgu o promieniu `r`.
Oś gniazda: równolegle do u albo v formatki (`face_socket(.., axis_along_u)`).

## 4. Zasady rysowania (zgodne z makietą)

- **Układ:** formatka (0,0) w lewym dolnym rogu; ekran ma oś Y w dół, więc `y_ekran = −y_maszyny`.
- **Transformacje widoku** (obrót 90°/180°, lustra, przerzut) stosuj do **gotowych punktów** macierzą afiniczną `[a,b,c,d,e,f]`
  (`apply()`). Nie obracaj kształtu parametrycznie. Dzięki temu łuki i otwory zawsze zostają spójne.
- **Wypełnienie:** kolor warstwy z alfą ≈ 0,2, obrys 1 px (stała grubość na ekranie, niezależna od zoomu).
- **Kolory (RGB hex):** nuta w krawędzi `#a98bd4`, gniazdo w powierzchni `#c79be6`, otwór pod klucz `#ecd2f7`.
- **Otwór pod klucz:** koło wypełnione `#0a0a0a`, obrys `#ecd2f7`, w środku kropka r = 1,4 px. Minimalny promień na ekranie 2,5 px.
  Rysowany tylko dla Clamex i tylko gdy reguła „Otwór pod klucz Clamex” jest włączona.
- **Widoczność:** nuta w krawędzi jest widoczna w każdym widoku (X-Ray / Góra / Dół). Gniazdo w powierzchni tylko na swojej stronie
  (przerzut Góra/Dół zamienia stronę), a w X-Ray widać obie.
- **Kolizje:** do wykrywania kolizji z łapami używaj prostokąta otaczającego (bbox) wszystkich punktów, razem z otworem.

## 5. Wartości domyślne (Ustawienia → Lamello P14)

| Klucz | Wartość | Źródło |
|---|---|---|
| `cutter_d` | 100,4 mm | rysunek i dokumentacja Lamello |
| `edge_depth` | 14 mm | rysunek Lamello |
| `face_depth` | 10 mm | rysunek Lamello |
| `sock_w` | 7 mm | rysunek (grubość tarczy) |
| `end_len` / `end_w` / `end_r` | 7 / 10 / 5 mm | Twoje wymiary (rysunek: 9,8 mm szerokości) |
| `key_d` | 6 mm (5–6) | rysunek: Ø5–6 |
| `key_off` | 7,5 mm | rysunek: 7,5 od krawędzi |
| głębokość otworu pod klucz | 13 mm | **założenie**, brak na rysunku |

## 6. Wartości kontrolne do testów

- `chord(50.2, 14)` = 69,56; `chord(50.2, 10)` = 60,13.
- `edge_slot_local(s0 = 270)`: 21 punktów, pierwszy `(235,22 ; 0)`, środkowy `(270 ; 14)`, ostatni `(304,78 ; 0)`.
- `edge_point(F3, 720, 560, 270, 14)` = `(270, 546)`; Clamex na F3: środek otworu `(270, 552,5)`, r = 3.
- `face_socket_local`: 34 punkty, a ∈ ±30,06, b ∈ ±5, w środku |b| = 3,5.
- `face_socket(u0 = 100, v0 = 150, axis_along_u = false)`: najmniejsze u = 95, rozpiętość v = 60,13.

## 7. Uwagi

- Rysunek Lamello podaje cięciwę gniazda ~60,3 mm. Wzór z tarczy daje 60,13 mm (różnica 0,2 mm, to przybliżenie producenta). Kod używa wzoru, więc obie nuty zawsze wynikają z jednej tarczy.
- W makiecie wariant Clamex/Tenso zmienia tylko otwór pod klucz. Gniazdo w powierzchni jest identyczne dla obu.
