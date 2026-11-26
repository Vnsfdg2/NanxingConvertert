use crate::model::{ConvertOptions, EdgeBanding, Operation, PanelProgram, Vec3};
use crate::step_parser::{NormalizedStepSolid, StepCircleEdge, StepCylinderFace};

#[derive(Debug, Clone)]
struct CylinderGroup {
    origin: Vec3,
    axis: Vec3,
    radius: f64,
    vertices: Vec<Vec3>,
    circle_edges: Vec<StepCircleEdge>,
}

pub fn detect_panel_program(
    solid: &NormalizedStepSolid,
    file_stem: &str,
    opts: &ConvertOptions,
) -> PanelProgram {
    let groups = group_coaxial_cylinders(&solid.cylinders);

    let mut lamello_ops: Vec<Operation> = Vec::new();
    let mut lamello_origins: Vec<(Vec3, Vec3)> = Vec::new(); // (origin, axis) to exclude companion R=47.5/48.66 necks
    let mut used_group_idx = vec![false; groups.len()];

    // 1. Detect Lamello P14 (R ≈ 50.2 mm)
    for (idx, g) in groups.iter().enumerate() {
        if (g.radius - 50.2).abs() <= 0.6 {
            if let Some(op) =
                classify_lamello_p14(g, solid.length, solid.width, solid.thickness, opts)
            {
                lamello_ops.push(op);
                lamello_origins.push((g.origin, g.axis));
                used_group_idx[idx] = true;
            }
        }
    }

    // Mark companion Lamello neck cylinders (e.g. TopSolid R=47.5 / R=48.66 co-axial with R=50.2)
    for (idx, g) in groups.iter().enumerate() {
        if used_group_idx[idx] {
            continue;
        }
        if g.radius >= 45.0 && g.radius <= 52.0 {
            for (lo, la) in &lamello_origins {
                if g.axis.dot(*la).abs() > 0.99 && point_to_line_dist(g.origin, *lo, *la) < 1.0 {
                    used_group_idx[idx] = true;
                    break;
                }
            }
        }
    }

    // 2. Detect vertical and horizontal drill holes
    let mut drill_ops: Vec<Operation> = Vec::new();
    for (idx, g) in groups.iter().enumerate() {
        if used_group_idx[idx] {
            continue;
        }
        let dia = g.radius * 2.0;
        if dia < 1.5 || dia > 45.0 {
            continue;
        }

        // Reject partial arcs (< 360°) unless explicitly enabled — except vertical Clamex access cylinders
        // sitting directly inside an already-detected edge Lamello P14 slot (in case CAD exported B-spline intersection edges)
        let over_edge_lamello = g.axis.z.abs() > 0.99
            && lamello_ops.iter().any(|lop| {
                if let Operation::LamelloP14 { face, x, y, .. } = *lop {
                    (1..=4).contains(&face)
                        && is_hole_in_edge_lamello_zone(
                            face,
                            x,
                            y,
                            g.origin.x,
                            g.origin.y,
                            dia,
                            solid.length,
                            solid.width,
                        )
                } else {
                    false
                }
            });
        if !opts.include_partial_holes && !over_edge_lamello && !is_full_360_hole(g) {
            continue;
        }

        let eff_dia = if opts.swap_d11_to_d12 && (dia - 11.0).abs() < 0.25 {
            12.0
        } else {
            round_dia(dia)
        };

        if g.axis.z.abs() > 0.99 {
            if let Some(op) = classify_vertical_hole(g, eff_dia, solid.thickness, opts) {
                drill_ops.push(op);
            }
        } else if g.axis.x.abs() > 0.99 || g.axis.y.abs() > 0.99 {
            if let Some(op) = classify_horizontal_hole(
                g,
                eff_dia,
                solid.length,
                solid.width,
                solid.thickness,
                opts,
            ) {
                drill_ops.push(op);
            }
        }
    }

    // 3. Determine whether panel needs automatic top/bottom flip (180° around Y axis)
    let mut flipped = false;
    let mut outline = solid.outline.clone();
    if opts.auto_flip_top_bottom
        && should_flip_top_bottom(&lamello_ops, &drill_ops, solid.length, solid.width, opts)
    {
        flipped = true;
        flip_operations_around_y(&mut lamello_ops, solid.length, solid.thickness);
        flip_operations_around_y(&mut drill_ops, solid.length, solid.thickness);
        for p in &mut outline {
            p.x = solid.length - p.x;
        }
        outline.reverse();
    }

    // 4. Dla każdego gniazda krawędziowego Lamello P14 (Face 1..4):
    //    - jeśli w bryle STEP w obrębie gniazda znajduje się pionowy otwór (Clamex P-14 z SolidCraft / TopSolid Wood),
    //      zachowujemy wiercenie od góry (Face 5) i dopilnowujemy głębokości do osi gniazda (T - z);
    //    - jeśli w bryle STEP w obrębie gniazda NIE MA otworu (np. Lamello Tenso P-14 zatrzaskowe),
    //      NIE dodajemy żadnego sztucznego wiercenia!
    let mut imbus_ops: Vec<Operation> = Vec::new();
    for op in &lamello_ops {
        if let Operation::LamelloP14 { face, x, y, z, .. } = *op {
            if (1..=4).contains(&face) {
                let mut found_hole: Option<(f64, f64, f64, f64, bool)> = None;
                drill_ops.retain(|d| {
                    if let Operation::VerticalDrill {
                        x: dx,
                        y: dy,
                        diameter,
                        depth,
                        is_through,
                        ..
                    } = *d
                    {
                        if is_hole_in_edge_lamello_zone(
                            face,
                            x,
                            y,
                            dx,
                            dy,
                            diameter,
                            solid.length,
                            solid.width,
                        ) {
                            if found_hole.is_none() {
                                found_hole = Some((dx, dy, diameter, depth, is_through));
                            }
                            return false;
                        }
                    }
                    true
                });

                if let Some((hx, hy, hdia, hdepth, is_through)) = found_hole {
                    let min_clamex_depth = (solid.thickness - z).clamp(1.0, solid.thickness);
                    let final_depth = if is_through {
                        solid.thickness
                    } else {
                        hdepth.max(min_clamex_depth).min(solid.thickness)
                    };
                    imbus_ops.push(Operation::VerticalDrill {
                        face: 5,
                        x: hx,
                        y: hy,
                        diameter: hdia,
                        depth: final_depth,
                        is_through,
                    });
                }
            }
        }
    }

    let mut operations = Vec::new();
    operations.extend(drill_ops);
    operations.extend(imbus_ops);
    operations.extend(lamello_ops);

    let short_id = extract_short_id(file_stem, solid.product_name.as_deref());

    let mut prog = PanelProgram {
        id: short_id,
        name: file_stem.to_string(),
        length: solid.length,
        width: solid.width,
        thickness: solid.thickness,
        material: opts.material.clone(),
        grain: opts.grain,
        outline,
        operations,
        edges: EdgeBanding::default(),
        warnings: Vec::new(),
        flipped_top_bottom: flipped,
    };

    // 5. Automatyczny obrót w osi Z (mniej otworów od strony łap na dłuższych bokach + kąt prosty 90° przy (0,0) i pełny bok dla kształtów L/skosów)
    if opts.auto_flip_top_bottom {
        crate::transforms::auto_orient_in_z(&mut prog, opts);
    } else {
        crate::transforms::recompute_panel_warnings(&mut prog, opts);
    }

    prog
}

/// Extract short barcode/QR ID (e.g. `S01_03`, `S14_15`, `SW_65`) regardless of token order,
/// checking the filename stem first and falling back to the internal STEP `PRODUCT` name.
pub fn extract_short_id(file_stem: &str, product_name: Option<&str>) -> String {
    if let Some(id) = parse_short_id_from_text(file_stem) {
        return id;
    }
    if let Some(prod) = product_name {
        if let Some(id) = parse_short_id_from_text(prod) {
            return id;
        }
    }
    file_stem.to_string()
}

fn parse_short_id_from_text(text: &str) -> Option<String> {
    let tokens: Vec<&str> = text
        .split(|c: char| c == '_' || c == ' ' || c == '-')
        .filter(|t| !t.is_empty())
        .collect();

    let mut cabinet: Option<(usize, String)> = None;
    let mut part_p: Option<String> = None;
    let mut pure_nums: Vec<(usize, &str)> = Vec::new();
    let mut is_wspolne = false;

    for (idx, &tok) in tokens.iter().enumerate() {
        let up = tok.to_ascii_uppercase();
        if up.starts_with('S') && up.len() >= 2 && up[1..].chars().all(|c| c.is_ascii_digit()) {
            if cabinet.is_none() {
                cabinet = Some((idx, up));
            }
        } else if up == "SWSPOLNE" || up == "WSP" || up == "WSPOLNE" {
            is_wspolne = true;
        } else if up.starts_with('P')
            && up.len() >= 2
            && up[1..].chars().all(|c| c.is_ascii_digit())
        {
            if part_p.is_none() {
                part_p = Some(up[1..].to_string());
            }
        } else if (1..=4).contains(&tok.len()) && tok.chars().all(|c| c.is_ascii_digit()) {
            pure_nums.push((idx, tok));
        }
    }

    if let Some((s_idx, s_code)) = cabinet {
        // Priority 1: explicit P-number anywhere (e.g. 14_Wieniec-polka_S14_P15 -> S14_15, P08_S13 -> S13_08)
        if let Some(p_num) = part_p {
            return Some(format!("{s_code}_{p_num}"));
        }
        // Priority 2: pure digits right after S-code (e.g. S01_03_18x364x400 -> S01_03)
        if let Some(&(_, num)) = pure_nums.iter().find(|&&(i, _)| i == s_idx + 1) {
            return Some(format!("{s_code}_{num}"));
        }
        // Priority 3: any pure digits token (e.g. 03_bok_S01 -> S01_03)
        if let Some(&(_, num)) = pure_nums.first() {
            return Some(format!("{s_code}_{num}"));
        }
        return Some(s_code);
    }

    if is_wspolne {
        if let Some(p_num) = part_p {
            return Some(format!("SW_{p_num}"));
        }
        if let Some(&(_, num)) = pure_nums.first() {
            return Some(format!("SW_{num}"));
        }
    }

    None
}

fn axial_interval(origin: Vec3, axis: Vec3, verts: &[Vec3]) -> Option<(f64, f64)> {
    if verts.is_empty() {
        return None;
    }
    let mut t_min = f64::INFINITY;
    let mut t_max = f64::NEG_INFINITY;
    for v in verts {
        let t = v.sub(origin).dot(axis);
        t_min = t_min.min(t);
        t_max = t_max.max(t);
    }
    Some((t_min, t_max))
}

fn group_coaxial_cylinders(cyls: &[StepCylinderFace]) -> Vec<CylinderGroup> {
    let mut groups: Vec<CylinderGroup> = Vec::new();
    for c in cyls {
        let mut matched = false;
        for g in &mut groups {
            if (g.radius - c.radius).abs() < 0.15
                && g.axis.dot(c.axis).abs() > 0.995
                && point_to_line_dist(c.origin, g.origin, g.axis) < 0.15
            {
                // Merge only if axial spans along the shared axis overlap or touch within 0.5 mm.
                // This prevents opposite-edge holes (e.g. Face 3 & Face 4 at identical Y, Z)
                // or opposite-face blind holes (Face 5 & Face 6) from collapsing into one group!
                let overlaps = match (
                    axial_interval(g.origin, g.axis, &g.vertices),
                    axial_interval(g.origin, g.axis, &c.vertices),
                ) {
                    (Some((g0, g1)), Some((c0, c1))) => g0 <= c1 + 0.5 && c0 <= g1 + 0.5,
                    _ => true,
                };
                if overlaps {
                    g.vertices.extend_from_slice(&c.vertices);
                    g.circle_edges.extend_from_slice(&c.circle_edges);
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            groups.push(CylinderGroup {
                origin: c.origin,
                axis: c.axis,
                radius: c.radius,
                vertices: c.vertices.clone(),
                circle_edges: c.circle_edges.clone(),
            });
        }
    }

    // Secondary merge pass in case 3+ segments arrived in non-contiguous order
    let mut merged = true;
    while merged {
        merged = false;
        'outer: for i in 0..groups.len() {
            for j in (i + 1)..groups.len() {
                let same_line = (groups[i].radius - groups[j].radius).abs() < 0.15
                    && groups[i].axis.dot(groups[j].axis).abs() > 0.995
                    && point_to_line_dist(groups[j].origin, groups[i].origin, groups[i].axis)
                        < 0.15;
                if !same_line {
                    continue;
                }
                let overlaps = match (
                    axial_interval(groups[i].origin, groups[i].axis, &groups[i].vertices),
                    axial_interval(groups[i].origin, groups[i].axis, &groups[j].vertices),
                ) {
                    (Some((a0, a1)), Some((b0, b1))) => a0 <= b1 + 0.5 && b0 <= a1 + 0.5,
                    _ => true,
                };
                if overlaps {
                    let other = groups.remove(j);
                    groups[i].vertices.extend(other.vertices);
                    groups[i].circle_edges.extend(other.circle_edges);
                    merged = true;
                    break 'outer;
                }
            }
        }
    }

    groups
}

fn point_to_line_dist(p: Vec3, line_pt: Vec3, line_dir: Vec3) -> f64 {
    let d = p.sub(line_pt);
    d.cross(line_dir).len()
}

fn is_full_360_hole(g: &CylinderGroup) -> bool {
    if g.circle_edges.iter().any(|e| e.is_full_circle) {
        return true;
    }
    // Check seam-split circles: sum of sweep angles at the same axial station >= 355°
    let mut stations: Vec<(f64, f64)> = Vec::new();
    for e in &g.circle_edges {
        if (e.radius - g.radius).abs() > 0.2 {
            continue;
        }
        let proj = e.center.sub(g.origin).dot(g.axis);
        if let Some(st) = stations.iter_mut().find(|(p, _)| (*p - proj).abs() < 0.25) {
            st.1 += e.sweep_deg;
        } else {
            stations.push((proj, e.sweep_deg));
        }
    }
    stations.iter().any(|(_, sweep)| *sweep >= 355.0)
}

fn classify_lamello_p14(
    g: &CylinderGroup,
    l: f64,
    w: f64,
    t: f64,
    opts: &ConvertOptions,
) -> Option<Operation> {
    if g.vertices.is_empty() {
        return None;
    }

    // Wyznacz rzeczywisty środek osiowy walca (w STEP CYLINDRICAL_SURFACE origin leży na jednej ze ścianek bocznych tarczy 10 mm!)
    let (t_min, t_max) = axial_interval(g.origin, g.axis, &g.vertices)?;
    let t_mid = 0.5 * (t_min + t_max);
    let mid = Vec3::new(
        g.origin.x + g.axis.x * t_mid,
        g.origin.y + g.axis.y * t_mid,
        g.origin.z + g.axis.z * t_mid,
    );

    // Surface Lamello P14 (enters top Z=T or bottom Z=0): axis is along X or Y, mid.z outside [0, T]
    if g.axis.z.abs() < 0.05 && (mid.z > t + 5.0 || mid.z < -5.0) {
        let (face, z_face, depth) = if mid.z > t {
            (5u8, t, g.radius - (mid.z - t))
        } else {
            (6u8, 0.0, g.radius - (-mid.z))
        };
        if !(10.0..=18.0).contains(&depth) {
            return None;
        }
        let direction = if g.axis.x.abs() > 0.9 { 'Y' } else { 'X' };
        return Some(Operation::LamelloP14 {
            face,
            direction,
            x: mid.x,
            y: mid.y,
            z: z_face,
            length: opts.lamello_slot_length,
            width: opts.lamello_surface_width,
            max_depth: opts.lamello_surface_depth,
        });
    }

    // Edge Lamello P14 (enters Face 1..4): axis is along Z, mid.z inside [0, T]
    if g.axis.z.abs() > 0.95 && mid.z > 1.0 && mid.z < t - 1.0 {
        let (face, direction, ex, ey, depth) = if mid.y > w + 5.0 {
            (1u8, 'X', mid.x, w, g.radius - (mid.y - w))
        } else if mid.y < -5.0 {
            (2u8, 'X', mid.x, 0.0, g.radius - (-mid.y))
        } else if mid.x > l + 5.0 {
            (3u8, 'Y', l, mid.y, g.radius - (mid.x - l))
        } else if mid.x < -5.0 {
            (4u8, 'Y', 0.0, mid.y, g.radius - (-mid.x))
        } else {
            return None;
        };

        if !(10.0..=18.0).contains(&depth) {
            return None;
        }

        return Some(Operation::LamelloP14 {
            face,
            direction,
            x: ex,
            y: ey,
            z: mid.z,
            length: opts.lamello_slot_length,
            width: opts.lamello_edge_width,
            max_depth: opts.lamello_edge_depth,
        });
    }

    None
}

fn classify_vertical_hole(
    g: &CylinderGroup,
    dia: f64,
    thickness: f64,
    opts: &ConvertOptions,
) -> Option<Operation> {
    if g.vertices.is_empty() {
        return None;
    }
    let z_min = g.vertices.iter().map(|v| v.z).fold(f64::INFINITY, f64::min);
    let z_max = g
        .vertices
        .iter()
        .map(|v| v.z)
        .fold(f64::NEG_INFINITY, f64::max);

    let touches_top = (z_max - thickness).abs() < 0.6;
    let touches_bot = z_min.abs() < 0.6;

    if touches_top && touches_bot {
        return Some(Operation::VerticalDrill {
            face: 5,
            x: g.origin.x,
            y: g.origin.y,
            diameter: dia,
            depth: thickness,
            is_through: true,
        });
    }

    let (face, raw_depth) = if touches_top {
        (5u8, thickness - z_min)
    } else if touches_bot {
        (6u8, z_max)
    } else {
        return None;
    };

    if raw_depth < 0.5 {
        return None;
    }

    let depth = apply_drill_tip_118(raw_depth, dia, opts.drill_tip_118_max_dia);
    Some(Operation::VerticalDrill {
        face,
        x: g.origin.x,
        y: g.origin.y,
        diameter: dia,
        depth,
        is_through: false,
    })
}

fn classify_horizontal_hole(
    g: &CylinderGroup,
    dia: f64,
    l: f64,
    w: f64,
    t: f64,
    opts: &ConvertOptions,
) -> Option<Operation> {
    if g.vertices.is_empty() {
        return None;
    }
    if g.origin.z < 1.0 || g.origin.z > t - 1.0 {
        return None;
    }

    let (face, ex, ey, raw_depth, inner_coord, axis_is_x) = if g.axis.x.abs() > 0.99 {
        let x_min = g.vertices.iter().map(|v| v.x).fold(f64::INFINITY, f64::min);
        let x_max = g
            .vertices
            .iter()
            .map(|v| v.x)
            .fold(f64::NEG_INFINITY, f64::max);
        if x_min.abs() < 0.6 && x_max < l - 0.6 {
            (4u8, 0.0, g.origin.y, x_max, x_max, true)
        } else if (x_max - l).abs() < 0.6 && x_min > 0.6 {
            (3u8, l, g.origin.y, l - x_min, x_min, true)
        } else {
            return None;
        }
    } else {
        let y_min = g.vertices.iter().map(|v| v.y).fold(f64::INFINITY, f64::min);
        let y_max = g
            .vertices
            .iter()
            .map(|v| v.y)
            .fold(f64::NEG_INFINITY, f64::max);
        if y_min.abs() < 0.6 && y_max < w - 0.6 {
            (2u8, g.origin.x, 0.0, y_max, y_max, false)
        } else if (y_max - w).abs() < 0.6 && y_min > 0.6 {
            (1u8, g.origin.x, w, w - y_min, y_min, false)
        } else {
            return None;
        }
    };

    if raw_depth < 1.0 {
        return None;
    }

    // Check if there is a circular bottom edge at inner_coord (otherwise it breaks into another hole like Minifix Ø15)
    let has_circular_bottom = g.circle_edges.iter().any(|e| {
        let coord = if axis_is_x { e.center.x } else { e.center.y };
        (coord - inner_coord).abs() < 0.4
    });

    let mut depth = if has_circular_bottom {
        apply_drill_tip_118(raw_depth, dia, opts.drill_tip_118_max_dia)
    } else {
        raw_depth + opts.horizontal_no_bottom_allowance
    };

    if depth > opts.max_side_drill_depth {
        depth = opts.max_side_drill_depth;
    }

    Some(Operation::HorizontalDrill {
        face,
        x: ex,
        y: ey,
        z: g.origin.z,
        diameter: dia,
        depth,
    })
}

fn apply_drill_tip_118(raw_depth: f64, dia: f64, max_dia: f64) -> f64 {
    if dia <= max_dia {
        raw_depth + (dia / 2.0) / (59.0_f64.to_radians().tan())
    } else {
        raw_depth
    }
}

fn round_dia(d: f64) -> f64 {
    (d * 10.0).round() / 10.0
}

/// Sprawdza, czy pionowy otwór (dx, dy, diameter) z modelu STEP (SolidCraft / TopSolid Wood)
/// leży w geometrycznym obrębie gniazda krawędziowego Lamello P14 (np. otwór pod klucz Clamex).
#[allow(clippy::too_many_arguments)]
pub fn is_hole_in_edge_lamello_zone(
    lam_face: u8,
    lam_x: f64,
    lam_y: f64,
    hole_x: f64,
    hole_y: f64,
    hole_dia: f64,
    panel_length: f64,
    panel_width: f64,
) -> bool {
    if !(4.0..=10.5).contains(&hole_dia) {
        return false;
    }
    let (along_dist, inward_dist) = match lam_face {
        1 => ((hole_x - lam_x).abs(), panel_width - hole_y), // Y = Width
        2 => ((hole_x - lam_x).abs(), hole_y),               // Y = 0
        3 => ((hole_y - lam_y).abs(), panel_length - hole_x), // X = Length
        4 => ((hole_y - lam_y).abs(), hole_x),               // X = 0
        _ => return false,
    };
    // Cięciwa gniazda P14 ma ~70 mm (±35 mm), głębokość ~14 mm; otwór Clamex wypada w strefie środkowej gniazda
    along_dist <= 28.0 && (1.5..=18.5).contains(&inward_dist)
}

fn should_flip_top_bottom(
    lamellos: &[Operation],
    drills: &[Operation],
    length: f64,
    width: f64,
    _opts: &ConvertOptions,
) -> bool {
    let lamello_f5 = lamellos
        .iter()
        .filter(|o| matches!(o, Operation::LamelloP14 { face: 5, .. }))
        .count();
    let lamello_f6 = lamellos
        .iter()
        .filter(|o| matches!(o, Operation::LamelloP14 { face: 6, .. }))
        .count();

    // 1a. Jeśli gniazda powierzchniowe Lamello P14 występują TYLKO po jednej stronie,
    //     ustawiamy tę stronę na górze (Face 5), aby wykonać całość w jednym programie SCX.
    if (lamello_f5 == 0) != (lamello_f6 == 0) {
        return lamello_f6 > 0;
    }

    // Zliczamy otwory pod klucz Clamex oraz wiercenia nieprzelotowe na obu płaszczyznach:
    let mut imbus_f5 = 0usize;
    let mut imbus_f6 = 0usize;
    for l in lamellos {
        if let Operation::LamelloP14 { face, x, y, .. } = *l {
            if (1..=4).contains(&face) {
                for d in drills {
                    if let Operation::VerticalDrill {
                        face: df,
                        x: dx,
                        y: dy,
                        diameter,
                        is_through: false,
                        ..
                    } = *d
                    {
                        if is_hole_in_edge_lamello_zone(
                            face, x, y, dx, dy, diameter, length, width,
                        ) {
                            if df == 5 {
                                imbus_f5 += 1;
                            } else if df == 6 {
                                imbus_f6 += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    let blind_f5 = drills
        .iter()
        .filter(|d| {
            matches!(
                d,
                Operation::VerticalDrill {
                    face: 5,
                    is_through: false,
                    ..
                }
            )
        })
        .count();
    let blind_f6 = drills
        .iter()
        .filter(|d| {
            matches!(
                d,
                Operation::VerticalDrill {
                    face: 6,
                    is_through: false,
                    ..
                }
            )
        })
        .count();

    // 1b. Jeśli gniazda powierzchniowe Lamello P14 są PO OBU STRONACH (powstaną 2 programy SCX),
    //     jako główną górną ścianę (Program 1) wybieramy ścianę z większą łączną liczbą operacji!
    if lamello_f5 > 0 && lamello_f6 > 0 {
        let total_f5 = lamello_f5 + imbus_f5 + blind_f5;
        let total_f6 = lamello_f6 + imbus_f6 + blind_f6;
        if total_f6 != total_f5 {
            return total_f6 > total_f5;
        }
        if lamello_f6 != lamello_f5 {
            return lamello_f6 > lamello_f5;
        }
    }

    // 2. Otwory pod klucz Clamex (w obrębie gniazd krawędziowych)
    if imbus_f6 != imbus_f5 {
        return imbus_f6 > imbus_f5;
    }

    // 3. Więcej nieprzelotowych wierceń pionowych na górze (Face 5)
    blind_f6 > blind_f5
}

/// Flip panel 180° around Y axis (matching Nanxing machine default "Odwróć według osi Y"):
/// x -> L - x, y -> y, z -> T - z, Face 5 <-> 6, Face 3 <-> 4
fn flip_operations_around_y(ops: &mut [Operation], length: f64, thickness: f64) {
    for op in ops {
        match op {
            Operation::VerticalDrill { face, x, .. } => {
                *x = length - *x;
                *face = if *face == 5 { 6 } else { 5 };
            }
            Operation::HorizontalDrill { face, x, z, .. } => {
                *x = length - *x;
                *z = thickness - *z;
                *face = match *face {
                    3 => 4,
                    4 => 3,
                    other => other,
                };
            }
            Operation::LamelloP14 { face, x, z, .. } => {
                *x = length - *x;
                *z = thickness - *z;
                *face = match *face {
                    5 => 6,
                    6 => 5,
                    3 => 4,
                    4 => 3,
                    other => other,
                };
            }
            Operation::Groove {
                face,
                x,
                end_x,
                z,
                end_z,
                ..
            } => {
                *x = length - *x;
                *end_x = length - *end_x;
                *z = thickness - *z;
                *end_z = thickness - *end_z;
                *face = if *face == 5 { 6 } else { 5 };
            }
        }
    }
}
