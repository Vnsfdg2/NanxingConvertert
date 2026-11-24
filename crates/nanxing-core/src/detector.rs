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

        // Reject partial arcs (< 360°) unless explicitly enabled
        if !opts.include_partial_holes && !is_full_360_hole(g) {
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
    }

    // 4. For each edge Lamello P14 (Face 1..4), replace any shallow STEP Ø6 hole with full technical Ø6 imbus hole on Face 5
    let mut imbus_ops: Vec<Operation> = Vec::new();
    for op in &lamello_ops {
        if let Operation::LamelloP14 { face, x, y, z, .. } = *op {
            if (1..=4).contains(&face) {
                let (ix, iy) = match face {
                    1 => (x, solid.width - opts.lamello_imbus_edge_offset), // Y = W
                    2 => (x, opts.lamello_imbus_edge_offset),               // Y = 0
                    3 => (solid.length - opts.lamello_imbus_edge_offset, y), // X = L
                    4 => (opts.lamello_imbus_edge_offset, y),               // X = 0
                    _ => continue,
                };
                // Remove any matching shallow Ø6 hole from STEP near (ix, iy)
                drill_ops.retain(|d| {
                    if let Operation::VerticalDrill {
                        x: dx,
                        y: dy,
                        diameter,
                        ..
                    } = *d
                    {
                        let close = (dx - ix).hypot(dy - iy) < 3.5;
                        let is_d6 = (diameter - opts.lamello_imbus_diameter).abs() < 0.4;
                        !(close && is_d6)
                    } else {
                        true
                    }
                });

                let imbus_depth = (solid.thickness - z).clamp(1.0, solid.thickness);
                imbus_ops.push(Operation::VerticalDrill {
                    face: 5,
                    x: ix,
                    y: iy,
                    diameter: opts.lamello_imbus_diameter,
                    depth: imbus_depth,
                    is_through: false,
                });
            }
        }
    }

    let mut operations = Vec::new();
    operations.extend(drill_ops);
    operations.extend(imbus_ops);
    operations.extend(lamello_ops);

    // 5. Machine safety warnings (Pr90011 = 45 mm min width, Pr90056 = 100 mm anti-pinch warning)
    let mut warnings = Vec::new();
    if solid.width < opts.min_panel_width {
        warnings.push(format!(
            "BŁĄD LIMITU: Szerokość formatki {:.1} mm < {:.1} mm (Pr90011)!",
            solid.width, opts.min_panel_width
        ));
    } else if solid.width < opts.anti_pinch_warning_width {
        warnings.push(format!(
            "OSTRZEŻENIE: Szerokość formatki {:.1} mm < {:.1} mm (alarm przytrzaśnięcia Pr90056).",
            solid.width, opts.anti_pinch_warning_width
        ));
    }

    let short_id = extract_short_id(file_stem, solid.product_name.as_deref());

    PanelProgram {
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
        warnings,
        flipped_top_bottom: flipped,
    }
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

    // Surface Lamello P14 (enters top Z=T or bottom Z=0): axis is along X or Y, origin.z outside [0, T]
    if g.axis.z.abs() < 0.05 && (g.origin.z > t + 5.0 || g.origin.z < -5.0) {
        let (face, z_face, depth) = if g.origin.z > t {
            (5u8, t, g.radius - (g.origin.z - t))
        } else {
            (6u8, 0.0, g.radius - (-g.origin.z))
        };
        if !(10.0..=18.0).contains(&depth) {
            return None;
        }
        let direction = if g.axis.x.abs() > 0.9 { 'Y' } else { 'X' };
        return Some(Operation::LamelloP14 {
            face,
            direction,
            x: g.origin.x,
            y: g.origin.y,
            z: z_face,
            length: opts.lamello_slot_length,
            width: opts.lamello_surface_width,
            max_depth: opts.lamello_surface_depth,
        });
    }

    // Edge Lamello P14 (enters Face 1..4): axis is along Z, origin.z inside [0, T]
    if g.axis.z.abs() > 0.95 && g.origin.z > 1.0 && g.origin.z < t - 1.0 {
        let (face, direction, ex, ey, depth) = if g.origin.y > w + 5.0 {
            (1u8, 'X', g.origin.x, w, g.radius - (g.origin.y - w))
        } else if g.origin.y < -5.0 {
            (2u8, 'X', g.origin.x, 0.0, g.radius - (-g.origin.y))
        } else if g.origin.x > l + 5.0 {
            (3u8, 'Y', l, g.origin.y, g.radius - (g.origin.x - l))
        } else if g.origin.x < -5.0 {
            (4u8, 'Y', 0.0, g.origin.y, g.radius - (-g.origin.x))
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
            z: g.origin.z,
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

fn should_flip_top_bottom(
    lamellos: &[Operation],
    drills: &[Operation],
    length: f64,
    width: f64,
    opts: &ConvertOptions,
) -> bool {
    // 1. Surface Lamello P14: only upper spindle (Face 5 / Z+) can mill surface Lamello slots
    let lamello_f5 = lamellos
        .iter()
        .filter(|o| matches!(o, Operation::LamelloP14 { face: 5, .. }))
        .count();
    let lamello_f6 = lamellos
        .iter()
        .filter(|o| matches!(o, Operation::LamelloP14 { face: 6, .. }))
        .count();
    if lamello_f6 != lamello_f5 {
        return lamello_f6 > lamello_f5;
    }

    // 2. Edge Lamello P14 with Ø6 imbus hole: keep side with more imbus holes facing Z+ (Face 5)
    let mut imbus_f5 = 0usize;
    let mut imbus_f6 = 0usize;
    for l in lamellos {
        if let Operation::LamelloP14 { face, x, y, .. } = *l {
            if (1..=4).contains(&face) {
                let (ix, iy) = match face {
                    1 => (x, width - opts.lamello_imbus_edge_offset),
                    2 => (x, opts.lamello_imbus_edge_offset),
                    3 => (length - opts.lamello_imbus_edge_offset, y),
                    4 => (opts.lamello_imbus_edge_offset, y),
                    _ => continue,
                };
                for d in drills {
                    if let Operation::VerticalDrill {
                        face: df,
                        x: dx,
                        y: dy,
                        diameter,
                        ..
                    } = *d
                    {
                        if (dx - ix).hypot(dy - iy) < 3.5
                            && (diameter - opts.lamello_imbus_diameter).abs() < 0.4
                        {
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
    if imbus_f6 != imbus_f5 {
        return imbus_f6 > imbus_f5;
    }

    // 3. Place the face with MORE blind vertical holes on Z+ (Face 5)
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
