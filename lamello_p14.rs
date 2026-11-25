//! Geometria Lamello P14 (Clamex / Tenso) do rysowania podglądu 2D i generowania operacji.
//! Zero zależności (tylko std). Odwzorowanie 1:1 makiety `nanxing-makieta.html`
//! (funkcje: sockOutline, gałąź 'lamello' i 'lamsock' w buildOps, edgePoint).
//!
//! Test:  rustc --edition 2021 --test lamello_p14.rs -o /tmp/lam && /tmp/lam
//!
//! UKŁADY WSPÓŁRZĘDNYCH
//!   * formatka:  (u, v) w mm, początek (0,0) w lewym dolnym rogu, u = długość L, v = szerokość W.
//!   * lokalny układ krawędzi: (s, d) — s wzdłuż krawędzi, d w głąb formatki (prostopadle do krawędzi).
//!   * lokalny układ gniazda:  (a, b) — a wzdłuż osi gniazda, b w poprzek; środek gniazda = (0,0).
//!
//! Krawędzie (jak w makiecie): F1 = dół (v=0), F2 = prawa (u=L), F3 = góra (v=W), F4 = lewa (u=0).
//! s rośnie: F1 → w prawo, F2 → w górę, F3 → w prawo, F4 → w górę.

pub type Pt = [f64; 2];

#[derive(Clone, Copy, Debug)]
pub struct P14 {
    /// Średnica tarczy Lamello P-System (jedna dla nuty w krawędzi i w powierzchni).
    pub cutter_d: f64, // 100.4
    /// Głębokość nuty w krawędzi (strzałka odcinka koła).
    pub edge_depth: f64, // 14.0
    /// Głębokość gniazda w powierzchni (strzałka odcinka koła).
    pub face_depth: f64, // 10.0
    /// Szerokość środka gniazda w powierzchni (= grubość tarczy).
    pub sock_w: f64, // 7.0
    /// Długość pogrubionych końców gniazda (wzdłuż osi).
    pub end_len: f64, // 7.0
    /// Szerokość pogrubionych końców.
    pub end_w: f64, // 10.0
    /// Promień zaokrąglenia zewnętrznych narożników końców (5 = połowa end_w => pełne półkole).
    pub end_r: f64, // 5.0
    /// Otwór pod klucz (tylko Clamex): średnica i odległość środka od krawędzi.
    pub key_d: f64, // 6.0 (5..6)
    pub key_off: f64, // 7.5
}

impl Default for P14 {
    fn default() -> Self {
        Self {
            cutter_d: 100.4,
            edge_depth: 14.0,
            face_depth: 10.0,
            sock_w: 7.0,
            end_len: 7.0,
            end_w: 10.0,
            end_r: 5.0,
            key_d: 6.0,
            key_off: 7.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Clamex, // ma otwór pod klucz
    Tenso,  // bez otworu
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    F1,
    F2,
    F3,
    F4,
}

/// Długość cięciwy odcinka koła o promieniu `r` i strzałce `h`.
/// chord = 2·√(r² − (r − h)²) = 2·r·sin(acos((r − h)/r)).  Dla r=50.2: h=14 → 69.56, h=10 → 60.1.
pub fn chord(r: f64, h: f64) -> f64 {
    let h = h.min(r * 0.95);
    2.0 * (r * r - (r - h) * (r - h)).sqrt()
}

/// Punkt z układu krawędzi (s, d) do układu formatki (u, v).
pub fn edge_point(edge: Edge, l: f64, w: f64, s: f64, d: f64) -> Pt {
    match edge {
        Edge::F1 => [s, d],
        Edge::F2 => [l - d, s],
        Edge::F3 => [s, w - d],
        Edge::F4 => [d, s],
    }
}

/// NUTA W KRAWĘDZI — odcinek koła (widok z góry), współrzędne lokalne (s, d).
/// Środek tarczy leży POZA formatką: (s0, edge_depth − R). Łuk biegnie od lewego końca cięciwy
/// przez najgłębszy punkt (d = edge_depth, s = s0) do prawego końca. Zamknięcie = cięciwa na krawędzi (d = 0).
/// `segments` = liczba odcinków łuku (makieta: 20); zwraca segments+1 punktów.
pub fn edge_slot_local(p: &P14, s0: f64, segments: usize) -> Vec<Pt> {
    let r = p.cutter_d / 2.0;
    let h = p.edge_depth.min(r * 0.95);
    let th = ((r - h) / r).acos(); // połowa kąta rozwarcia
    (0..=segments)
        .map(|k| {
            let a = -th + 2.0 * th * k as f64 / segments as f64;
            [s0 + r * a.sin(), h - r + r * a.cos()]
        })
        .collect()
}

/// Ta sama nuta w układzie formatki (u, v). Wielokąt jest zamknięty niejawnie (ostatni → pierwszy = cięciwa).
pub fn edge_slot(p: &P14, edge: Edge, l: f64, w: f64, s0: f64, segments: usize) -> Vec<Pt> {
    edge_slot_local(p, s0, segments)
        .into_iter()
        .map(|q| edge_point(edge, l, w, q[0], q[1]))
        .collect()
}

/// OTWÓR POD KLUCZ (Clamex): środek na osi nuty, `key_off` od krawędzi. Zwraca (środek, promień).
/// Dla Tenso zwraca None.
pub fn key_hole(p: &P14, v: Variant, edge: Edge, l: f64, w: f64, s0: f64) -> Option<(Pt, f64)> {
    (v == Variant::Clamex).then(|| (edge_point(edge, l, w, s0, p.key_off), p.key_d / 2.0))
}

/// GNIAZDO W POWIERZCHNI — wąski środek + pogrubione końce z pełnym półkolem (kształt „kości”).
/// Współrzędne lokalne (a, b), środek (0,0), a wzdłuż osi gniazda. Wielokąt zamknięty niejawnie.
/// Długość całkowita = chord(R, face_depth) ≈ 60.1 mm.
/// Kolejność: dół-środek → prawy koniec (łuk) → góra-środek → lewy koniec (łuk) → powrót.
pub fn face_socket_local(p: &P14) -> Vec<Pt> {
    let l = chord(p.cutter_d / 2.0, p.face_depth);
    let (h, nw, e2) = (l / 2.0, p.sock_w / 2.0, p.end_w / 2.0);
    let el = p.end_len;
    let r = p.end_r.min(el).min(e2).max(0.0);
    let mut o: Vec<Pt> = Vec::with_capacity(34);
    // łuk o środku (cx,cy) i promieniu r, kąt a0→a1, 5 odcinków (6 punktów) — jak w makiecie
    let arc = |o: &mut Vec<Pt>, cx: f64, cy: f64, a0: f64, a1: f64| {
        if r <= 0.0 {
            return;
        }
        for i in 0..=5 {
            let a = a0 + (a1 - a0) * i as f64 / 5.0;
            o.push([cx + r * a.cos(), cy + r * a.sin()]);
        }
    };
    use std::f64::consts::PI;
    o.extend([[-h + el, -nw], [h - el, -nw], [h - el, -e2]]);
    if r > 0.0 {
        o.push([h - r, -e2]);
        arc(&mut o, h - r, -e2 + r, -PI / 2.0, 0.0);
        arc(&mut o, h - r, e2 - r, 0.0, PI / 2.0);
        o.push([h - el, e2]);
    } else {
        o.extend([[h, -e2], [h, e2], [h - el, e2]]);
    }
    o.extend([[h - el, nw], [-h + el, nw], [-h + el, e2]]);
    if r > 0.0 {
        o.push([-h + r, e2]);
        arc(&mut o, -h + r, e2 - r, PI / 2.0, PI);
        arc(&mut o, -h + r, -e2 + r, PI, 1.5 * PI);
        o.push([-h + el, -e2]);
    } else {
        o.extend([[-h, e2], [-h, -e2], [-h + el, -e2]]);
    }
    o
}

/// Gniazdo w układzie formatki. `axis_along_u`: true → oś gniazda równoległa do u, false → do v.
/// (u0, v0) = środek gniazda.
pub fn face_socket(p: &P14, u0: f64, v0: f64, axis_along_u: bool) -> Vec<Pt> {
    face_socket_local(p)
        .into_iter()
        .map(|q| if axis_along_u { [u0 + q[0], v0 + q[1]] } else { [u0 + q[1], v0 + q[0]] })
        .collect()
}

/// Transformacja afiniczna 2D [a,b,c,d,e,f]: x' = a·x + c·y + e,  y' = b·x + d·y + f
/// (ta sama konwencja co macierz `M` w makiecie: obroty 90/180°, lustra, przerzut).
pub type Affine = [f64; 6];
pub fn apply(m: &Affine, p: Pt) -> Pt {
    [m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]]
}

/// Ekran SVG/egui ma oś Y w dół: przy rysowaniu użyj y_screen = −y_machine (makieta: `Y=y=>-y`).
#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn chords_match_lamello_drawing() {
        let r = 100.4 / 2.0;
        assert!(close(chord(r, 14.0), 69.56, 0.01)); // rysunek: ~69.6
        assert!(close(chord(r, 10.0), 60.13, 0.01)); // rysunek: ~60.3 (przybliżenie producenta)
    }

    #[test]
    fn edge_slot_endpoints_on_edge_and_apex_at_depth() {
        let p = P14::default();
        let pts = edge_slot_local(&p, 270.0, 20);
        assert_eq!(pts.len(), 21);
        // końce cięciwy leżą na krawędzi (d = 0), symetrycznie wokół s0
        assert!(close(pts[0][1], 0.0, 1e-9) && close(pts[20][1], 0.0, 1e-9));
        assert!(close(pts[0][0], 270.0 - 34.78, 0.01) && close(pts[20][0], 270.0 + 34.78, 0.01));
        // środek łuku = najgłębszy punkt
        assert!(close(pts[10][0], 270.0, 1e-9) && close(pts[10][1], 14.0, 1e-9));
    }

    #[test]
    fn edge_mapping() {
        // F3 (góra) formatki 720×560: d=0 → v=560, d>0 → v mniejsze
        assert_eq!(edge_point(Edge::F3, 720.0, 560.0, 270.0, 14.0), [270.0, 546.0]);
        assert_eq!(edge_point(Edge::F4, 764.0, 540.0, 130.0, 7.5), [7.5, 130.0]);
        assert_eq!(edge_point(Edge::F2, 764.0, 540.0, 130.0, 7.5), [756.5, 130.0]);
    }

    #[test]
    fn key_hole_only_for_clamex() {
        let p = P14::default();
        assert!(key_hole(&p, Variant::Tenso, Edge::F3, 720.0, 560.0, 270.0).is_none());
        let (c, r) = key_hole(&p, Variant::Clamex, Edge::F3, 720.0, 560.0, 270.0).unwrap();
        assert_eq!(c, [270.0, 552.5]);
        assert_eq!(r, 3.0);
    }

    #[test]
    fn face_socket_shape() {
        let p = P14::default();
        let o = face_socket_local(&p);
        assert_eq!(o.len(), 34);
        let (mut amin, mut amax, mut bmin, mut bmax) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for q in &o {
            amin = amin.min(q[0]);
            amax = amax.max(q[0]);
            bmin = bmin.min(q[1]);
            bmax = bmax.max(q[1]);
        }
        let l = chord(50.2, 10.0);
        assert!(close(amin, -l / 2.0, 1e-9) && close(amax, l / 2.0, 1e-9));
        assert!(close(bmin, -5.0, 1e-9) && close(bmax, 5.0, 1e-9)); // końce 10 mm
        // środek wąski: punkt (a=0) ma |b| = 3.5 (7 mm)
        assert!(o.iter().any(|q| close(q[0].abs(), l / 2.0 - 7.0, 1e-9) && close(q[1].abs(), 3.5, 1e-9)));
    }

    #[test]
    fn face_socket_axis_y() {
        let p = P14::default();
        let o = face_socket(&p, 100.0, 150.0, false); // oś wzdłuż v
        let vmin = o.iter().map(|q| q[1]).fold(f64::MAX, f64::min);
        let vmax = o.iter().map(|q| q[1]).fold(f64::MIN, f64::max);
        assert!(close(vmax - vmin, 60.13, 0.01));
        let umin = o.iter().map(|q| q[0]).fold(f64::MAX, f64::min);
        assert!(close(umin, 95.0, 1e-9));
    }
}
