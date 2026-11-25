use crate::model::{ConvertOptions, EdgeBanding, GrainDirection, Operation, PanelProgram, Point2};

/// Transformacje orientacji formatki na stole 6-stronnej wiertarki Nanxing
/// (układ podglądu: punkt (0,0) w prawym górnym narożniku przy bolcu, łapy u góry Y=0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTransform {
    /// Obrót o +90° (zgodnie z ruchem wskazówek zegara w widoku z góry)
    Rotate90Plus,
    /// Obrót o −90° (przeciwnie do ruchu wskazówek zegara w widoku z góry)
    Rotate90Minus,
    /// Lustrzane odbicie Góra / Dół (oś pozioma: Y -> W - Y, bez zmiany strony Z)
    MirrorTopBottom,
    /// Lustrzane odbicie Prawo / Lewo (oś pionowa: X -> L - X, bez zmiany strony Z)
    MirrorLeftRight,
    /// Fizyczny przewrót formatki do góry nogami (Góra↔Dół Z + obrót 180° względem łap:
    /// X -> X, Y -> W - Y, Z -> T - Z; otwory przy łapach przechodzą na przeciwny bok i są wiercone od dołu)
    FlipUpsideDown,
}

impl PanelTransform {
    pub fn short_chip_label(self) -> &'static str {
        match self {
            Self::Rotate90Plus => "+90°",
            Self::Rotate90Minus => "−90°",
            Self::MirrorTopBottom => "Lustro G/D",
            Self::MirrorLeftRight => "Lustro P/L",
            Self::FlipUpsideDown => "Do góry nogami",
        }
    }

    pub fn inverse(self) -> Self {
        match self {
            Self::Rotate90Plus => Self::Rotate90Minus,
            Self::Rotate90Minus => Self::Rotate90Plus,
            Self::MirrorTopBottom => Self::MirrorTopBottom,
            Self::MirrorLeftRight => Self::MirrorLeftRight,
            Self::FlipUpsideDown => Self::FlipUpsideDown,
        }
    }
}

/// Dopisuje krok transformacji z uproszczeniem oczywistych cofnięć (np. +90° i −90° albo podwójne lustro).
pub fn push_simplified_transform(steps: &mut Vec<PanelTransform>, next: PanelTransform) {
    if let Some(&last) = steps.last() {
        let cancels = matches!(
            (last, next),
            (PanelTransform::Rotate90Plus, PanelTransform::Rotate90Minus)
                | (PanelTransform::Rotate90Minus, PanelTransform::Rotate90Plus)
                | (PanelTransform::MirrorTopBottom, PanelTransform::MirrorTopBottom)
                | (PanelTransform::MirrorLeftRight, PanelTransform::MirrorLeftRight)
                | (PanelTransform::FlipUpsideDown, PanelTransform::FlipUpsideDown)
        );
        if cancels {
            steps.pop();
            return;
        }
    }
    steps.push(next);
    // 4 obroty o 90° w tę samą stronę = pełne 360° (powrót)
    if steps.len() >= 4 {
        let n = steps.len();
        if steps[n - 4..]
            .iter()
            .all(|&s| s == PanelTransform::Rotate90Plus)
            || steps[n - 4..]
                .iter()
                .all(|&s| s == PanelTransform::Rotate90Minus)
        {
            steps.truncate(n - 4);
        }
    }
}

/// Formatuje zwięzły napis na chip w lewym panelu opisujący wykonaną zmianę orientacji.
pub fn format_transforms_chip(steps: &[PanelTransform]) -> Option<String> {
    if steps.is_empty() {
        return None;
    }
    let mut parts: Vec<&'static str> = Vec::new();
    let mut i = 0;
    while i < steps.len() {
        if i + 2 < steps.len()
            && steps[i] == PanelTransform::Rotate90Plus
            && steps[i + 1] == PanelTransform::Rotate90Plus
            && steps[i + 2] == PanelTransform::Rotate90Plus
        {
            parts.push("−90°");
            i += 3;
        } else if i + 2 < steps.len()
            && steps[i] == PanelTransform::Rotate90Minus
            && steps[i + 1] == PanelTransform::Rotate90Minus
            && steps[i + 2] == PanelTransform::Rotate90Minus
        {
            parts.push("+90°");
            i += 3;
        } else if i + 1 < steps.len()
            && ((steps[i] == PanelTransform::Rotate90Plus
                && steps[i + 1] == PanelTransform::Rotate90Plus)
                || (steps[i] == PanelTransform::Rotate90Minus
                    && steps[i + 1] == PanelTransform::Rotate90Minus))
        {
            parts.push("180°");
            i += 2;
        } else {
            parts.push(steps[i].short_chip_label());
            i += 1;
        }
    }
    Some(parts.join(" · "))
}

/// Aplikuje pojedynczą transformację do `PanelProgram` (oraz opcjonalnie do tablicy ręcznych obrzeży `[F1, F2, F3, F4]`).
pub fn apply_panel_transform(
    panel: &mut PanelProgram,
    edge_manual: Option<&mut [bool; 4]>,
    xf: PanelTransform,
    opts: &ConvertOptions,
) {
    let old_l = panel.length;
    let old_w = panel.width;
    let t = panel.thickness;

    let map_xy = |x: f64, y: f64| -> (f64, f64) {
        match xf {
            PanelTransform::Rotate90Plus => (y, old_l - x),
            PanelTransform::Rotate90Minus => (old_w - y, x),
            PanelTransform::MirrorTopBottom => (x, old_w - y),
            PanelTransform::MirrorLeftRight => (old_l - x, y),
            PanelTransform::FlipUpsideDown => (x, old_w - y),
        }
    };

    let map_side_face = |f: u8| -> u8 {
        match xf {
            PanelTransform::Rotate90Plus => match f {
                2 => 4, // Góra -> Prawo
                4 => 1, // Prawo -> Dół
                1 => 3, // Dół -> Lewo
                3 => 2, // Lewo -> Góra
                other => other,
            },
            PanelTransform::Rotate90Minus => match f {
                2 => 3, // Góra -> Lewo
                3 => 1, // Lewo -> Dół
                1 => 4, // Dół -> Prawo
                4 => 2, // Prawo -> Góra
                other => other,
            },
            PanelTransform::MirrorTopBottom | PanelTransform::FlipUpsideDown => match f {
                1 => 2,
                2 => 1,
                other => other,
            },
            PanelTransform::MirrorLeftRight => match f {
                3 => 4,
                4 => 3,
                other => other,
            },
        }
    };

    // 1. Wymiary i usłojenie
    if matches!(xf, PanelTransform::Rotate90Plus | PanelTransform::Rotate90Minus) {
        panel.length = old_w;
        panel.width = old_l;
        panel.grain = match panel.grain {
            GrainDirection::Length => GrainDirection::Width,
            GrainDirection::Width => GrainDirection::Length,
            GrainDirection::None => GrainDirection::None,
        };
    }

    // 2. Kontur <Outline>
    for p in &mut panel.outline {
        let (nx, ny) = map_xy(p.x, p.y);
        p.x = clean_zero(nx);
        p.y = clean_zero(ny);
    }
    if matches!(
        xf,
        PanelTransform::MirrorTopBottom
            | PanelTransform::MirrorLeftRight
            | PanelTransform::FlipUpsideDown
    ) {
        panel.outline.reverse();
    }

    // 3. Obrzeża <EdgeGroup> (1=Dół, 2=Góra, 3=Lewo, 4=Prawo)
    let old_edges = panel.edges.clone();
    let mut new_edges = EdgeBanding::default();
    for old_f in 1..=4u8 {
        let new_f = map_side_face(old_f);
        new_edges.set(new_f, old_edges.get(old_f));
    }
    panel.edges = new_edges;

    if let Some(m) = edge_manual {
        let old_m = *m;
        let mut new_m = [false; 4];
        for old_f in 1..=4u8 {
            let new_f = map_side_face(old_f);
            new_m[(new_f - 1) as usize] = old_m[(old_f - 1) as usize];
        }
        *m = new_m;
    }

    // 4. Operacje <Machining>
    let swap_dir = matches!(xf, PanelTransform::Rotate90Plus | PanelTransform::Rotate90Minus);
    let flip_z = xf == PanelTransform::FlipUpsideDown;

    for op in &mut panel.operations {
        match op {
            Operation::HorizontalDrill {
                face, x, y, z, ..
            } => {
                let (nx, ny) = map_xy(*x, *y);
                *x = clean_zero(nx);
                *y = clean_zero(ny);
                *face = map_side_face(*face);
                if flip_z {
                    *z = clean_zero(t - *z);
                }
            }
            Operation::VerticalDrill {
                face,
                x,
                y,
                is_through,
                ..
            } => {
                let (nx, ny) = map_xy(*x, *y);
                *x = clean_zero(nx);
                *y = clean_zero(ny);
                if flip_z && !*is_through {
                    *face = if *face == 5 { 6 } else { 5 };
                }
            }
            Operation::LamelloP14 {
                face,
                direction,
                x,
                y,
                z,
                ..
            } => {
                let (nx, ny) = map_xy(*x, *y);
                *x = clean_zero(nx);
                *y = clean_zero(ny);
                if swap_dir {
                    *direction = if *direction == 'X' { 'Y' } else { 'X' };
                }
                if (1..=4).contains(face) {
                    *face = map_side_face(*face);
                    if flip_z {
                        *z = clean_zero(t - *z);
                    }
                } else if flip_z {
                    *face = if *face == 5 { 6 } else { 5 };
                    *z = if *face == 5 { t } else { 0.0 };
                }
            }
            Operation::Groove {
                face,
                x,
                y,
                z,
                end_x,
                end_y,
                end_z,
                ..
            } => {
                let (nx, ny) = map_xy(*x, *y);
                let (nex, ney) = map_xy(*end_x, *end_y);
                *x = clean_zero(nx);
                *y = clean_zero(ny);
                *end_x = clean_zero(nex);
                *end_y = clean_zero(ney);
                if flip_z {
                    *face = if *face == 5 { 6 } else { 5 };
                    *z = clean_zero(t - *z);
                    *end_z = clean_zero(t - *end_z);
                }
            }
        }
    }

    if flip_z {
        panel.flipped_top_bottom = !panel.flipped_top_bottom;
    }

    recompute_panel_warnings(panel, opts);
}

/// Przelicza ostrzeżenia technologiczne po każdej zmianie orientacji lub ustawień.
pub fn recompute_panel_warnings(panel: &mut PanelProgram, opts: &ConvertOptions) {
    panel.warnings.clear();

    if panel.width < opts.min_panel_width {
        panel.warnings.push(format!(
            "BŁĄD LIMITU: Szerokość {:.1} mm < {:.1} mm (Pr90011)!",
            panel.width, opts.min_panel_width
        ));
    } else if panel.width < opts.anti_pinch_warning_width {
        panel.warnings.push(format!(
            "OSTRZEŻENIE: Szerokość {:.1} mm < {:.1} mm (alarm przytrzaśnięcia Pr90056).",
            panel.width, opts.anti_pinch_warning_width
        ));
    }

    if panel.length < 140.0 && panel.width > panel.length * 1.35 {
        panel.warnings.push(format!(
            "UWAGA: Krótki bok ({:.0} mm) od strony łap — obróć formatkę o 90°.",
            panel.length
        ));
    }

    let has_f6_lamello = panel
        .operations
        .iter()
        .any(|o| matches!(o, Operation::LamelloP14 { face: 6, .. }));
    if has_f6_lamello {
        panel.warnings.push(
            "UWAGA: Gniazdo powierzchniowe Lamello P14 znajduje się od spodu (Face 6)!".to_string(),
        );
    }

    let has_bad_groove = panel.operations.iter().any(|o| {
        if let Operation::Groove { y, end_y, .. } = *o {
            (end_y - y).abs() > 0.05
        } else {
            false
        }
    });
    if has_bad_groove {
        panel
            .warnings
            .push("UWAGA: Nuta piłą nie leży w osi X — obróć formatkę o 90°.".to_string());
    }
}

/// Automatycznie dobiera optymalny obrót w osi Z (0°, +90°, 180°, −90°) przy wczytywaniu formatki:
/// 1. Dla kształtów nieprostokątnych (L, skosy, wycięcia):
///    - wymaga kąta prostego 90° w prawym górnym narożniku `(0,0)` (dosunięcie do rolek na łapach Y=0 i bolca z prawej X=0),
///    - wymaga podparcia pod obiema łapami i preferuje najdłuższy prosty bok bez ścięć i wycięć od strony łap (Y=0).
/// 2. Dla prostokątów:
///    - ustawia wyłącznie dłuższy bok od strony łap (np. 500×120 nigdy nie stanie bokiem 120 mm do łap; dla kwadratów dopuszcza wszystkie 4 boki),
///    - minimalizuje liczbę wierceń bocznych / gniazd na krawędzi od strony łap (`Face 2`, Y=0).
pub fn auto_orient_in_z(panel: &mut PanelProgram, opts: &ConvertOptions) {
    let is_rect = is_axis_aligned_rectangle(&panel.outline, panel.length, panel.width);

    let mut candidates: Vec<(usize, PanelProgram)> = Vec::with_capacity(4);
    let mut cur = panel.clone();
    for rot_steps in 0..4usize {
        if rot_steps > 0 {
            apply_panel_transform(&mut cur, None, PanelTransform::Rotate90Plus, opts);
        }
        candidates.push((rot_steps, cur.clone()));
    }

    let best_idx = candidates
        .iter()
        .enumerate()
        .max_by_key(|(_, (rot_steps, cand))| score_z_candidate(cand, is_rect, *rot_steps, opts))
        .map(|(idx, _)| idx)
        .unwrap_or(0);

    if best_idx > 0 {
        *panel = candidates.swap_remove(best_idx).1;
    } else {
        recompute_panel_warnings(panel, opts);
    }
}

fn score_z_candidate(
    cand: &PanelProgram,
    is_rect: bool,
    rot_steps: usize,
    opts: &ConvertOptions,
) -> (bool, bool, bool, bool, i64, bool, i64, i64) {
    let l = cand.length;
    let w = cand.width;

    // 1. Szerokość powyżej minimalnego limitu maszyny Pr90011 (45 mm)
    let valid_min_width = w + 1e-3 >= opts.min_panel_width;

    // Analiza krawędzi przy łapach (Y = 0) oraz prawego bolca (X = 0) dla narożnika (0,0)
    let geom = analyze_datum_edges(&cand.outline, l, w);

    // Liczba otworów bocznych / gniazd od strony łap (Face 2, Y = 0) — im mniej, tym lepiej (ujemna do max_by_key)
    let ops_at_clamps = cand
        .operations
        .iter()
        .filter(|op| {
            matches!(
                op,
                Operation::HorizontalDrill { face: 2, .. } | Operation::LamelloP14 { face: 2, .. }
            )
        })
        .count() as i64;

    // Zgodność ewentualnych nut piłą z osią X
    let grooves_ok = !cand.operations.iter().any(|op| {
        if let Operation::Groove { y, end_y, .. } = *op {
            (end_y - y).abs() > 0.05
        } else {
            false
        }
    });

    // Preferuj brak zbędnego obrotu w razie pełnego remisu
    let tie_break = -(rot_steps as i64);

    if is_rect {
        // Dla prostokątów: TYLKO dłuższe boki do łap (np. 500×120 -> L=500 >= W=120; kwadrat 400×400 -> wszystkie 4 boki)
        let is_longer_side = l + 0.5 >= w;
        (
            valid_min_width,
            is_longer_side,
            true,
            true,
            0,
            grooves_ok,
            -ops_at_clamps,
            tie_break,
        )
    } else {
        // Dla kształtów nieprostokątnych (L, skosy, wycięcia):
        // - kąt prosty w (0,0) (styk z rolkami Y=0 oraz prawym bolcem X=0),
        // - podparcie obu łap i brak wycięcia/ścięcia na krawędzi łap (pełny prosty odcinek na Y=0),
        // - najdłuższy prosty bok bez ścięć i wycięć od strony łap (zaokrąglony do 1 mm),
        // - ale nie kosztem wąskiego paska < 110 mm przy łapach jeśli istnieje dłuższy bok!
        let not_too_narrow_for_clamps = l >= 130.0 || l + 0.5 >= w;
        let straight_top_mm = (geom.longest_top_segment_len * 10.0).round() as i64;
        (
            valid_min_width,
            not_too_narrow_for_clamps && geom.clamps_supported,
            geom.square_top_right_corner,
            geom.full_unbroken_top_edge,
            straight_top_mm,
            grooves_ok,
            -ops_at_clamps,
            tie_break,
        )
    }
}

struct DatumEdgeAnalysis {
    square_top_right_corner: bool,
    clamps_supported: bool,
    full_unbroken_top_edge: bool,
    longest_top_segment_len: f64,
}

fn analyze_datum_edges(outline: &[Point2], length: f64, _width: f64) -> DatumEdgeAnalysis {
    if outline.len() < 3 {
        return DatumEdgeAnalysis {
            square_top_right_corner: true,
            clamps_supported: true,
            full_unbroken_top_edge: true,
            longest_top_segment_len: length,
        };
    }

    const TOL: f64 = 0.6;
    // Zbierz odcinki leżące na górnej linii łap (Y = 0) oraz prawej linii bolca (X = 0)
    let mut top_intervals: Vec<(f64, f64)> = Vec::new();
    let mut right_intervals: Vec<(f64, f64)> = Vec::new();

    let n = outline.len();
    for i in 0..n {
        let a = outline[i];
        let b = outline[(i + 1) % n];
        if a.y.abs() <= TOL && b.y.abs() <= TOL && (a.x - b.x).abs() > 1e-3 {
            top_intervals.push((a.x.min(b.x), a.x.max(b.x)));
        }
        if a.x.abs() <= TOL && b.x.abs() <= TOL && (a.y - b.y).abs() > 1e-3 {
            right_intervals.push((a.y.min(b.y), a.y.max(b.y)));
        }
    }

    let merged_top = merge_1d_intervals(top_intervals);
    let merged_right = merge_1d_intervals(right_intervals);

    let longest_top = merged_top
        .iter()
        .map(|(x0, x1)| x1 - x0)
        .fold(0.0_f64, f64::max);

    // Pełny bok bez ścięć i wycięć od X=0 do X=Length
    let full_unbroken_top = merged_top.len() == 1
        && merged_top[0].0 <= TOL + 0.5
        && (length - merged_top[0].1) <= TOL + 0.5;

    // Kąt prosty w narożniku bazowym (0,0): krawędź Y=0 zaczyna się w X=0 (min 25 mm styku)
    // ORAZ krawędź X=0 zaczyna się w Y=0 (min 25 mm styku z bolcem pozycjonującym)
    let min_ref = 25.0_f64.min(length * 0.4);
    let top_touches_origin = merged_top
        .iter()
        .any(|&(x0, x1)| x0 <= TOL + 0.5 && x1 >= min_ref);
    let right_touches_origin = merged_right
        .iter()
        .any(|&(y0, y1)| y0 <= TOL + 0.5 && y1 >= 18.0);
    let square_top_right = top_touches_origin && right_touches_origin;

    // Sprawdź, czy obie łapy (w okolicach 20% i 80% długości) mają materiał na krawędzi Y=0
    let c1 = length * 0.20;
    let c2 = length * 0.80;
    let clamp1_ok = merged_top
        .iter()
        .any(|&(x0, x1)| x0 <= c1 + 10.0 && x1 >= c1 - 10.0 && (x1 - x0) >= 35.0);
    let clamp2_ok = merged_top
        .iter()
        .any(|&(x0, x1)| x0 <= c2 + 10.0 && x1 >= c2 - 10.0 && (x1 - x0) >= 35.0);

    DatumEdgeAnalysis {
        square_top_right_corner: square_top_right,
        clamps_supported: clamp1_ok && clamp2_ok,
        full_unbroken_top_edge: full_unbroken_top,
        longest_top_segment_len: longest_top,
    }
}

fn merge_1d_intervals(mut ivs: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    if ivs.is_empty() {
        return ivs;
    }
    ivs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = vec![ivs[0]];
    for (a, b) in ivs.into_iter().skip(1) {
        let last = out.last_mut().unwrap();
        if a <= last.1 + 1.0 {
            last.1 = last.1.max(b);
        } else {
            out.push((a, b));
        }
    }
    out
}

fn is_axis_aligned_rectangle(outline: &[Point2], length: f64, width: f64) -> bool {
    if outline.is_empty() {
        return true;
    }
    let a = analyze_datum_edges(outline, length, width);
    if !a.full_unbroken_top_edge || !a.square_top_right_corner {
        return false;
    }
    // Sprawdź także przeciwległe dwa boki (Y = Width oraz X = Length)
    const TOL: f64 = 0.6;
    let mut bot = Vec::new();
    let mut left = Vec::new();
    let mut right = Vec::new();
    let n = outline.len();
    for i in 0..n {
        let p = outline[i];
        let q = outline[(i + 1) % n];
        // Każdy punkt prostokąta leży na obwodzie ramki [0..L] × [0..W]
        let on_box = p.x.abs() <= TOL
            || (p.x - length).abs() <= TOL
            || p.y.abs() <= TOL
            || (p.y - width).abs() <= TOL;
        if !on_box {
            return false;
        }
        if (p.y - width).abs() <= TOL && (q.y - width).abs() <= TOL && (p.x - q.x).abs() > 1e-3 {
            bot.push((p.x.min(q.x), p.x.max(q.x)));
        }
        if (p.x - length).abs() <= TOL && (q.x - length).abs() <= TOL && (p.y - q.y).abs() > 1e-3 {
            left.push((p.y.min(q.y), p.y.max(q.y)));
        }
        if p.x.abs() <= TOL && q.x.abs() <= TOL && (p.y - q.y).abs() > 1e-3 {
            right.push((p.y.min(q.y), p.y.max(q.y)));
        }
    }
    let m_bot = merge_1d_intervals(bot);
    let m_left = merge_1d_intervals(left);
    let m_right = merge_1d_intervals(right);

    m_bot.len() == 1
        && (m_bot[0].1 - m_bot[0].0 - length).abs() <= 1.2
        && m_left.len() == 1
        && (m_left[0].1 - m_left[0].0 - width).abs() <= 1.2
        && m_right.len() == 1
        && (m_right[0].1 - m_right[0].0 - width).abs() <= 1.2
}

fn clean_zero(v: f64) -> f64 {
    if v.abs() < 1e-7 { 0.0 } else { v }
}
