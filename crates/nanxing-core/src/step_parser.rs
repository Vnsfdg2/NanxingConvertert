use std::collections::HashMap;
use std::f64::consts::PI;

use crate::model::{Point2, Vec3};

#[derive(Debug, Clone)]
pub struct StepCircleEdge {
    pub center: Vec3,
    pub axis: Vec3,
    pub radius: f64,
    pub v_start: Vec3,
    pub v_end: Vec3,
    pub is_full_circle: bool,
    pub sweep_deg: f64,
}

#[derive(Debug, Clone)]
pub struct StepCylinderFace {
    pub face_id: u32,
    pub origin: Vec3,
    pub axis: Vec3,
    pub radius: f64,
    pub vertices: Vec<Vec3>,
    pub circle_edges: Vec<StepCircleEdge>,
}

#[derive(Debug, Clone)]
pub struct StepPlaneFace {
    pub face_id: u32,
    pub origin: Vec3,
    pub normal: Vec3,
    pub vertices: Vec<Vec3>,
}

#[derive(Debug, Clone)]
pub struct NormalizedStepSolid {
    pub product_name: Option<String>,
    pub length: f64,
    pub width: f64,
    pub thickness: f64,
    pub outline: Vec<Point2>,
    pub cylinders: Vec<StepCylinderFace>,
    pub planes: Vec<StepPlaneFace>,
}

struct StepDoc {
    entities: HashMap<u32, (String, String)>,
}

impl StepDoc {
    fn parse(content: &str) -> Result<Self, String> {
        let mut entities = HashMap::new();
        let data_start = content.find("DATA;").map(|i| i + 5).unwrap_or(0);
        let data_end = content[data_start..]
            .find("ENDSEC;")
            .map(|i| data_start + i)
            .unwrap_or(content.len());
        let slice = &content[data_start..data_end];

        // Remove /* ... */ comments and split by ';' outside single quotes
        let bytes = slice.as_bytes();
        let mut clean = String::with_capacity(slice.len());
        let mut i = 0;
        let mut in_quote = false;
        while i < bytes.len() {
            if !in_quote && i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
            let ch = bytes[i] as char;
            if ch == '\'' {
                in_quote = !in_quote;
            }
            if ch == ';' && !in_quote {
                Self::parse_statement(&clean, &mut entities);
                clean.clear();
            } else if ch != '\r' && ch != '\n' {
                clean.push(ch);
            }
            i += 1;
        }
        if !clean.trim().is_empty() {
            Self::parse_statement(&clean, &mut entities);
        }

        if entities.is_empty() {
            return Err("Nie znaleziono encji STEP w sekcji DATA.".to_string());
        }
        Ok(Self { entities })
    }

    fn parse_statement(stmt: &str, out: &mut HashMap<u32, (String, String)>) {
        let s = stmt.trim();
        if !s.starts_with('#') {
            return;
        }
        let Some(eq_pos) = s.find('=') else { return };
        let id_str = s[1..eq_pos].trim();
        let Ok(id) = id_str.parse::<u32>() else { return };
        let rhs = s[eq_pos + 1..].trim();
        let Some(paren_pos) = rhs.find('(') else { return };
        let type_name = rhs[..paren_pos].trim().to_ascii_uppercase();
        if !rhs.ends_with(')') {
            return;
        }
        let args = rhs[paren_pos + 1..rhs.len() - 1].trim().to_string();
        out.insert(id, (type_name, args));
    }

    fn split_top_level(s: &str) -> Vec<String> {
        let mut res = Vec::new();
        let mut depth = 0i32;
        let mut in_quote = false;
        let mut start = 0usize;
        let bytes = s.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            match b {
                b'\'' => in_quote = !in_quote,
                b'(' if !in_quote => depth += 1,
                b')' if !in_quote => depth -= 1,
                b',' if !in_quote && depth == 0 => {
                    res.push(s[start..i].trim().to_string());
                    start = i + 1;
                }
                _ => {}
            }
        }
        res.push(s[start..].trim().to_string());
        res
    }

    fn parse_ref(tok: &str) -> Option<u32> {
        let t = tok.trim();
        t.strip_prefix('#')?.parse::<u32>().ok()
    }

    fn parse_ref_list(tok: &str) -> Vec<u32> {
        let t = tok.trim();
        let inner = t
            .strip_prefix('(')
            .and_then(|x| x.strip_suffix(')'))
            .unwrap_or(t);
        inner.split(',').filter_map(Self::parse_ref).collect()
    }

    fn parse_f64_tuple(tok: &str) -> Option<Vec3> {
        let t = tok.trim();
        let inner = t.strip_prefix('(')?.strip_suffix(')')?;
        let nums: Vec<f64> = inner
            .split(',')
            .filter_map(|x| x.trim().parse::<f64>().ok())
            .collect();
        if nums.len() == 3 {
            Some(Vec3::new(nums[0], nums[1], nums[2]))
        } else {
            None
        }
    }

    fn get_point(&self, id: u32) -> Option<Vec3> {
        let (ty, args) = self.entities.get(&id)?;
        if ty != "CARTESIAN_POINT" {
            return None;
        }
        let parts = Self::split_top_level(args);
        Self::parse_f64_tuple(parts.get(1)?)
    }

    fn get_direction(&self, id: u32) -> Option<Vec3> {
        let (ty, args) = self.entities.get(&id)?;
        if ty != "DIRECTION" {
            return None;
        }
        let parts = Self::split_top_level(args);
        Some(Self::parse_f64_tuple(parts.get(1)?)?.normalized())
    }

    fn get_axis2(&self, id: u32) -> Option<(Vec3, Vec3, Vec3)> {
        let (ty, args) = self.entities.get(&id)?;
        if ty != "AXIS2_PLACEMENT_3D" {
            return None;
        }
        let parts = Self::split_top_level(args);
        let origin = self.get_point(Self::parse_ref(parts.get(1)?)?)?;
        let z_dir = parts
            .get(2)
            .and_then(|s| Self::parse_ref(s))
            .and_then(|r| self.get_direction(r))
            .unwrap_or(Vec3::new(0.0, 0.0, 1.0));
        let x_dir = parts
            .get(3)
            .and_then(|s| Self::parse_ref(s))
            .and_then(|r| self.get_direction(r))
            .unwrap_or(Vec3::new(1.0, 0.0, 0.0));
        Some((origin, z_dir, x_dir))
    }

    fn get_vertex(&self, id: u32) -> Option<Vec3> {
        let (ty, args) = self.entities.get(&id)?;
        if ty != "VERTEX_POINT" {
            return None;
        }
        let parts = Self::split_top_level(args);
        self.get_point(Self::parse_ref(parts.get(1)?)?)
    }

    fn get_bound_edges(&self, bound_id: u32) -> Vec<(u32, bool)> {
        let Some((ty, args)) = self.entities.get(&bound_id) else {
            return Vec::new();
        };
        if ty != "FACE_OUTER_BOUND" && ty != "FACE_BOUND" {
            return Vec::new();
        }
        let parts = Self::split_top_level(args);
        let Some(loop_id) = parts.get(1).and_then(|s| Self::parse_ref(s)) else {
            return Vec::new();
        };
        let bound_sense = parts.get(2).map(|s| s.contains(".T.")).unwrap_or(true);
        let Some((lty, largs)) = self.entities.get(&loop_id) else {
            return Vec::new();
        };
        if lty != "EDGE_LOOP" {
            return Vec::new();
        }
        let lparts = Self::split_top_level(largs);
        let Some(oe_list) = lparts.get(1) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for oe_id in Self::parse_ref_list(oe_list) {
            if let Some((oty, oargs)) = self.entities.get(&oe_id) {
                if oty == "ORIENTED_EDGE" {
                    let op = Self::split_top_level(oargs);
                    if let Some(ec_id) = op.get(3).and_then(|s| Self::parse_ref(s)) {
                        let oe_sense = op.get(4).map(|s| s.contains(".T.")).unwrap_or(true);
                        out.push((ec_id, bound_sense == oe_sense));
                    }
                }
            }
        }
        out
    }

    fn get_edge_curve(&self, ec_id: u32) -> Option<(Vec3, Vec3, u32, bool)> {
        let (ty, args) = self.entities.get(&ec_id)?;
        if ty != "EDGE_CURVE" {
            return None;
        }
        let parts = Self::split_top_level(args);
        let v1 = self.get_vertex(Self::parse_ref(parts.get(1)?)?)?;
        let v2 = self.get_vertex(Self::parse_ref(parts.get(2)?)?)?;
        let geom_id = Self::parse_ref(parts.get(3)?)?;
        let same_sense = parts.get(4).map(|s| s.contains(".T.")).unwrap_or(true);
        Some((v1, v2, geom_id, same_sense))
    }

    fn get_circle(&self, geom_id: u32) -> Option<(Vec3, Vec3, f64)> {
        let (ty, args) = self.entities.get(&geom_id)?;
        if ty != "CIRCLE" {
            return None;
        }
        let parts = Self::split_top_level(args);
        let (origin, axis, _) = self.get_axis2(Self::parse_ref(parts.get(1)?)?)?;
        let radius = parts.get(2)?.trim().parse::<f64>().ok()?;
        Some((origin, axis, radius))
    }
}

struct PanelFrame {
    origin: Vec3,
    ex: Vec3,
    ey: Vec3,
    ez: Vec3,
}

impl PanelFrame {
    fn to_local_pt(&self, p: Vec3) -> Vec3 {
        let d = p.sub(self.origin);
        Vec3::new(d.dot(self.ex), d.dot(self.ey), d.dot(self.ez))
    }

    fn to_local_dir(&self, d: Vec3) -> Vec3 {
        Vec3::new(d.dot(self.ex), d.dot(self.ey), d.dot(self.ez)).normalized()
    }
}

pub fn parse_and_normalize_step(content: &str) -> Result<NormalizedStepSolid, String> {
    let doc = StepDoc::parse(content)?;

    // 1. Collect all vertices and raw planar faces to compute a true 3D Oriented Bounding Box (AutoAlign)
    let mut all_verts = Vec::new();
    for (&id, (ty, _)) in &doc.entities {
        if ty == "VERTEX_POINT" {
            if let Some(p) = doc.get_vertex(id) {
                all_verts.push(p);
            }
        }
    }
    if all_verts.len() < 8 {
        return Err("Za mało wierzchołków w pliku STEP, aby zidentyfikować formatkę.".to_string());
    }

    struct RawPlaneInfo {
        normal: Vec3,
        offset: f64,
        area: f64,
        outer_edges: Vec<(Vec3, Vec3)>,
    }

    let mut raw_planes: Vec<RawPlaneInfo> = Vec::new();
    for (ty, args) in doc.entities.values() {
        if ty != "ADVANCED_FACE" {
            continue;
        }
        let parts = StepDoc::split_top_level(args);
        let Some(bounds_tok) = parts.get(1) else {
            continue;
        };
        let Some(surf_id) = parts.get(2).and_then(|s| StepDoc::parse_ref(s)) else {
            continue;
        };
        let Some((sty, sargs)) = doc.entities.get(&surf_id) else {
            continue;
        };
        if sty != "PLANE" {
            continue;
        }
        let sp = StepDoc::split_top_level(sargs);
        let Some(ax_id) = sp.get(1).and_then(|s| StepDoc::parse_ref(s)) else {
            continue;
        };
        let Some((raw_orig, raw_norm, _)) = doc.get_axis2(ax_id) else {
            continue;
        };
        let normal = raw_norm.normalized();
        if normal.len() < 0.5 {
            continue;
        }

        let bound_ids = StepDoc::parse_ref_list(bounds_tok);
        let outer_bid = bound_ids
            .iter()
            .copied()
            .find(|&bid| {
                doc.entities
                    .get(&bid)
                    .map(|(bty, _)| bty == "FACE_OUTER_BOUND")
                    .unwrap_or(false)
            })
            .or_else(|| bound_ids.first().copied());

        let mut outer_pts = Vec::new();
        let mut outer_edges = Vec::new();
        if let Some(bid) = outer_bid {
            for (ec_id, sense) in doc.get_bound_edges(bid) {
                if let Some((v1, v2, _, esense)) = doc.get_edge_curve(ec_id) {
                    let (a, b) = if sense == esense { (v1, v2) } else { (v2, v1) };
                    outer_pts.push(a);
                    outer_edges.push((a, b));
                }
            }
        }
        if outer_pts.len() < 3 {
            continue;
        }
        let area = polygon_area_3d(&outer_pts, normal);
        if area < 1e-3 {
            continue;
        }
        raw_planes.push(RawPlaneInfo {
            normal,
            offset: raw_orig.dot(normal),
            area,
            outer_edges,
        });
    }

    // 1a. Find true panel thickness axis `ez`:
    // Group planar faces by parallel normal (modulo sign) and pick the axis whose opposing
    // top/bottom extremal skins have the largest combined area (works for tilted, beveled, and mitered boards).
    let mut candidate_normals: Vec<Vec3> = Vec::new();
    for rp in &raw_planes {
        let cn = canonical_dir(rp.normal);
        if !candidate_normals
            .iter()
            .any(|e| e.dot(cn).abs() > 0.998)
        {
            candidate_normals.push(cn);
        }
    }

    let mut best_ez: Option<Vec3> = None;
    let mut best_skin_score = -1.0f64;
    let mut best_skin_span = f64::INFINITY;

    for &u in &candidate_normals {
        let (mut p_min, mut p_max) = (f64::INFINITY, f64::NEG_INFINITY);
        for &v in &all_verts {
            let p = v.dot(u);
            p_min = p_min.min(p);
            p_max = p_max.max(p);
        }
        let span = p_max - p_min;
        if span < 1.5 {
            continue;
        }

        let mut area_bot = 0.0f64;
        let mut area_top = 0.0f64;
        for rp in &raw_planes {
            let dot = rp.normal.dot(u);
            if dot.abs() <= 0.998 {
                continue;
            }
            let signed_off = if dot >= 0.0 { rp.offset } else { -rp.offset };
            if (signed_off - p_min).abs() < 0.45 {
                area_bot += rp.area;
            } else if (signed_off - p_max).abs() < 0.45 {
                area_top += rp.area;
            }
        }

        // Must have real opposing top & bottom faces (not a single mitered face)
        if area_bot < 10.0 || area_top < 10.0 {
            continue;
        }

        let combined_area = area_bot + area_top;
        // Pick largest opposing skin area; if almost tied (e.g., square prism), prefer thinner span
        let better = combined_area > best_skin_score * 1.02
            || ((combined_area - best_skin_score).abs() <= best_skin_score * 0.02
                && span < best_skin_span);
        if better {
            best_skin_score = combined_area;
            best_skin_span = span;
            best_ez = Some(u);
        }
    }

    let ez_raw = best_ez.unwrap_or_else(|| {
        // Fallback to smallest global axis if no opposing planar faces found
        let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut min_z, mut max_z) = (f64::INFINITY, f64::NEG_INFINITY);
        for v in &all_verts {
            min_x = min_x.min(v.x);
            max_x = max_x.max(v.x);
            min_y = min_y.min(v.y);
            max_y = max_y.max(v.y);
            min_z = min_z.min(v.z);
            max_z = max_z.max(v.z);
        }
        let dims = [
            (max_x - min_x, Vec3::new(1.0, 0.0, 0.0)),
            (max_y - min_y, Vec3::new(0.0, 1.0, 0.0)),
            (max_z - min_z, Vec3::new(0.0, 0.0, 1.0)),
        ];
        dims.iter()
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .map(|d| d.1)
            .unwrap_or(Vec3::new(0.0, 0.0, 1.0))
    });

    // 1b. Find optimal in-plane axes (ex, ey) perpendicular to ez_raw via minimum-area 2D OBB
    // over all straight edge directions of the top/bottom faces (aligns slanted & mitered strips cleanly,
    // and breaks area ties by preferring frames with maximum total orthogonal edge support `u` + `v`).
    let mut top_bot_edges: Vec<(Vec3, f64)> = Vec::new();
    let mut candidate_dirs: Vec<Vec3> = Vec::new();
    for rp in &raw_planes {
        if rp.normal.dot(ez_raw).abs() > 0.995 {
            for &(a, b) in &rp.outer_edges {
                let d_raw = b.sub(a);
                let proj = d_raw.sub(scale_v(ez_raw, d_raw.dot(ez_raw)));
                let len = proj.len();
                if len < 5.0 {
                    continue;
                }
                let dir = proj.normalized();
                top_bot_edges.push((dir, len));
                if !candidate_dirs
                    .iter()
                    .any(|e| e.dot(dir).abs() > 0.9995 || e.cross(dir).len() < 0.03)
                {
                    candidate_dirs.push(dir);
                }
            }
        }
    }
    // Also include projections of world axes as fallback candidates
    for w in [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ] {
        let proj = w.sub(scale_v(ez_raw, w.dot(ez_raw)));
        if proj.len() > 0.1 {
            candidate_dirs.push(proj.normalized());
        }
    }

    let mut best_ex = Vec3::new(1.0, 0.0, 0.0);
    let mut best_ey = Vec3::new(0.0, 1.0, 0.0);
    let mut best_obb_area = f64::INFINITY;
    let mut best_ortho_support = -1.0f64;

    for dir in candidate_dirs {
        let u = dir.normalized();
        let v = ez_raw.cross(u).normalized();
        if v.len() < 0.5 {
            continue;
        }
        let (mut u0, mut u1) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut v0, mut v1) = (f64::INFINITY, f64::NEG_INFINITY);
        for &pt in &all_verts {
            let pu = pt.dot(u);
            let pv = pt.dot(v);
            u0 = u0.min(pu);
            u1 = u1.max(pu);
            v0 = v0.min(pv);
            v1 = v1.max(pv);
        }
        let su = u1 - u0;
        let sv = v1 - v0;
        let area = su * sv;

        // Total length of outer edges aligned with EITHER u or v (prefers 90-degree corners over hypotenuse!)
        let ortho_support: f64 = top_bot_edges
            .iter()
            .filter(|(ed, _)| ed.dot(u).abs() > 0.999 || ed.dot(v).abs() > 0.999)
            .map(|(_, l)| *l)
            .sum();

        // Choose minimum-area 2D bounding box; on near-ties (< 0.2%), prefer higher orthogonal edge support
        let better = area < best_obb_area * 0.998
            || ((area - best_obb_area).abs() <= best_obb_area * 0.002
                && ortho_support > best_ortho_support);
        if better {
            best_obb_area = area;
            best_ortho_support = ortho_support;
            if su >= sv {
                best_ex = u;
                best_ey = v;
            } else {
                best_ex = v;
                best_ey = Vec3::new(-u.x, -u.y, -u.z);
            }
        }
    }

    let mut ex_raw = best_ex.normalized();
    let ey_raw = best_ey.normalized();
    // Ensure right-handed basis (ex x ey == ez)
    if ex_raw.cross(ey_raw).dot(ez_raw) < 0.0 {
        ex_raw = Vec3::new(-ex_raw.x, -ex_raw.y, -ex_raw.z);
    }

    // Compute local origin so all vertices are zeroed into [0, L] x [0, W] x [0, T]
    let temp_frame = PanelFrame {
        origin: Vec3::new(0.0, 0.0, 0.0),
        ex: ex_raw,
        ey: ey_raw,
        ez: ez_raw,
    };
    let (mut lx_min, mut lx_max) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut ly_min, mut ly_max) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut lz_min, mut lz_max) = (f64::INFINITY, f64::NEG_INFINITY);
    for &v in &all_verts {
        let p = temp_frame.to_local_pt(v);
        lx_min = lx_min.min(p.x);
        lx_max = lx_max.max(p.x);
        ly_min = ly_min.min(p.y);
        ly_max = ly_max.max(p.y);
        lz_min = lz_min.min(p.z);
        lz_max = lz_max.max(p.z);
    }

    let origin = Vec3::new(
        ex_raw.x * lx_min + ey_raw.x * ly_min + ez_raw.x * lz_min,
        ex_raw.y * lx_min + ey_raw.y * ly_min + ez_raw.y * lz_min,
        ex_raw.z * lx_min + ey_raw.z * ly_min + ez_raw.z * lz_min,
    );
    let frame = PanelFrame {
        origin,
        ex: ex_raw,
        ey: ey_raw,
        ez: ez_raw,
    };

    let length = (lx_max - lx_min).abs();
    let width = (ly_max - ly_min).abs();
    let thickness = (lz_max - lz_min).abs();

    // 2. Extract all CYLINDRICAL_SURFACE and PLANE advanced faces
    let mut cylinders = Vec::new();
    let mut planes = Vec::new();
    let mut top_face_outer_bound: Option<u32> = None;
    let mut top_face_best_span = 0.0f64;

    for (&face_id, (ty, args)) in &doc.entities {
        if ty != "ADVANCED_FACE" {
            continue;
        }
        let parts = StepDoc::split_top_level(args);
        let Some(bounds_tok) = parts.get(1) else {
            continue;
        };
        let Some(surf_id) = parts.get(2).and_then(|s| StepDoc::parse_ref(s)) else {
            continue;
        };
        let Some((sty, sargs)) = doc.entities.get(&surf_id) else {
            continue;
        };
        let bound_ids = StepDoc::parse_ref_list(bounds_tok);

        if sty == "CYLINDRICAL_SURFACE" {
            let sp = StepDoc::split_top_level(sargs);
            let Some(ax_id) = sp.get(1).and_then(|s| StepDoc::parse_ref(s)) else {
                continue;
            };
            let Some(radius) = sp.get(2).and_then(|s| s.trim().parse::<f64>().ok()) else {
                continue;
            };
            let Some((raw_orig, raw_axis, _)) = doc.get_axis2(ax_id) else {
                continue;
            };
            let origin_l = frame.to_local_pt(raw_orig);
            let axis_l = frame.to_local_dir(raw_axis);

            let mut verts = Vec::new();
            let mut circle_edges = Vec::new();
            for &bid in &bound_ids {
                for (ec_id, _sense) in doc.get_bound_edges(bid) {
                    if let Some((v1, v2, geom_id, _esense)) = doc.get_edge_curve(ec_id) {
                        let lv1 = frame.to_local_pt(v1);
                        let lv2 = frame.to_local_pt(v2);
                        verts.push(lv1);
                        verts.push(lv2);

                        if let Some((c_orig, c_axis, c_rad)) = doc.get_circle(geom_id) {
                            let lc = frame.to_local_pt(c_orig);
                            let la = frame.to_local_dir(c_axis);
                            let dist_ends = lv1.sub(lv2).len();
                            let is_full = dist_ends < 1e-3;
                            let sweep_deg = if is_full {
                                360.0
                            } else {
                                let r1 = lv1.sub(lc).normalized();
                                let r2 = lv2.sub(lc).normalized();
                                let cos_a = r1.dot(r2).clamp(-1.0, 1.0);
                                cos_a.acos() * 180.0 / PI
                            };
                            circle_edges.push(StepCircleEdge {
                                center: lc,
                                axis: la,
                                radius: c_rad,
                                v_start: lv1,
                                v_end: lv2,
                                is_full_circle: is_full,
                                sweep_deg,
                            });
                        }
                    }
                }
            }

            cylinders.push(StepCylinderFace {
                face_id,
                origin: origin_l,
                axis: axis_l,
                radius,
                vertices: verts,
                circle_edges,
            });
        } else if sty == "PLANE" {
            let sp = StepDoc::split_top_level(sargs);
            let Some(ax_id) = sp.get(1).and_then(|s| StepDoc::parse_ref(s)) else {
                continue;
            };
            let Some((raw_orig, raw_norm, _)) = doc.get_axis2(ax_id) else {
                continue;
            };
            let origin_l = frame.to_local_pt(raw_orig);
            let normal_l = frame.to_local_dir(raw_norm);

            let mut verts = Vec::new();
            let mut outer_bid = None;
            for &bid in &bound_ids {
                if let Some((bty, _)) = doc.entities.get(&bid) {
                    if bty == "FACE_OUTER_BOUND" {
                        outer_bid = Some(bid);
                    }
                }
                for (ec_id, _) in doc.get_bound_edges(bid) {
                    if let Some((v1, v2, _, _)) = doc.get_edge_curve(ec_id) {
                        verts.push(frame.to_local_pt(v1));
                        verts.push(frame.to_local_pt(v2));
                    }
                }
            }

            // Track largest top/bottom planar face for real 2D outline extraction
            if normal_l.z.abs() > 0.99
                && ((origin_l.z - thickness).abs() < 0.35 || origin_l.z.abs() < 0.35)
                && !verts.is_empty()
            {
                let (mut fx0, mut fx1) = (f64::INFINITY, f64::NEG_INFINITY);
                let (mut fy0, mut fy1) = (f64::INFINITY, f64::NEG_INFINITY);
                for v in &verts {
                    fx0 = fx0.min(v.x);
                    fx1 = fx1.max(v.x);
                    fy0 = fy0.min(v.y);
                    fy1 = fy1.max(v.y);
                }
                let span = (fx1 - fx0) * (fy1 - fy0);
                if span > top_face_best_span {
                    top_face_best_span = span;
                    top_face_outer_bound = outer_bid;
                }
            }

            planes.push(StepPlaneFace {
                face_id,
                origin: origin_l,
                normal: normal_l,
                vertices: verts,
            });
        }
    }

    let outline = extract_outline(&doc, &frame, top_face_outer_bound, length, width);

    // Extract internal STEP PRODUCT name (if present) for ID parsing fallback
    let product_name = doc
        .entities
        .values()
        .find(|(ty, _)| ty == "PRODUCT")
        .and_then(|(_, args)| {
            let parts = StepDoc::split_top_level(args);
            parts.first().map(|s| s.trim().trim_matches('\'').to_string())
        })
        .filter(|s| !s.is_empty());

    Ok(NormalizedStepSolid {
        product_name,
        length,
        width,
        thickness,
        outline,
        cylinders,
        planes,
    })
}

fn extract_outline(
    doc: &StepDoc,
    frame: &PanelFrame,
    outer_bound_id: Option<u32>,
    length: f64,
    width: f64,
) -> Vec<Point2> {
    let fallback = vec![
        Point2::new(length, width),
        Point2::new(0.0, width),
        Point2::new(0.0, 0.0),
        Point2::new(length, 0.0),
    ];

    let Some(bid) = outer_bound_id else {
        return fallback;
    };
    let edges = doc.get_bound_edges(bid);
    if edges.len() < 3 {
        return fallback;
    }

    let mut pts: Vec<Point2> = Vec::new();
    for (ec_id, sense) in edges {
        let Some((v1_raw, v2_raw, geom_id, esense)) = doc.get_edge_curve(ec_id) else {
            continue;
        };
        let forward = sense == esense;
        let (p_start, p_end) = if forward {
            (frame.to_local_pt(v1_raw), frame.to_local_pt(v2_raw))
        } else {
            (frame.to_local_pt(v2_raw), frame.to_local_pt(v1_raw))
        };

        // Sample circle arcs on outer contour with intermediate points so arcs render smoothly
        if let Some((c_raw, a_raw, c_rad)) = doc.get_circle(geom_id) {
            let c = frame.to_local_pt(c_raw);
            let a = frame.to_local_dir(a_raw);
            if a.z.abs() > 0.95 && c_rad > 1.0 && p_start.sub(p_end).len() > 1e-3 {
                let a0 = (p_start.y - c.y).atan2(p_start.x - c.x);
                let a1 = (p_end.y - c.y).atan2(p_end.x - c.x);
                let mut da = a1 - a0;
                // Choose shortest arc sweep if <= 180 deg
                while da > PI {
                    da -= 2.0 * PI;
                }
                while da < -PI {
                    da += 2.0 * PI;
                }
                let steps = ((da.abs() * 180.0 / PI) / 10.0).ceil().clamp(2.0, 24.0) as usize;
                for s in 0..steps {
                    let t = s as f64 / steps as f64;
                    let ang = a0 + da * t;
                    let pt = Point2::new(c.x + c_rad * ang.cos(), c.y + c_rad * ang.sin());
                    push_unique_pt(&mut pts, pt);
                }
                continue;
            }
        }

        push_unique_pt(&mut pts, Point2::new(p_start.x, p_start.y));
    }

    if pts.len() < 3 {
        fallback
    } else {
        pts
    }
}

fn push_unique_pt(pts: &mut Vec<Point2>, p: Point2) {
    if let Some(last) = pts.last() {
        if (last.x - p.x).abs() < 1e-3 && (last.y - p.y).abs() < 1e-3 {
            return;
        }
    }
    pts.push(p);
}

fn scale_v(v: Vec3, s: f64) -> Vec3 {
    Vec3::new(v.x * s, v.y * s, v.z * s)
}

fn canonical_dir(v: Vec3) -> Vec3 {
    let n = v.normalized();
    if n.z < -1e-6
        || (n.z.abs() <= 1e-6 && n.y < -1e-6)
        || (n.z.abs() <= 1e-6 && n.y.abs() <= 1e-6 && n.x < 0.0)
    {
        Vec3::new(-n.x, -n.y, -n.z)
    } else {
        n
    }
}

fn polygon_area_3d(pts: &[Vec3], normal: Vec3) -> f64 {
    if pts.len() < 3 {
        return 0.0;
    }
    let p0 = pts[0];
    let mut sum = Vec3::new(0.0, 0.0, 0.0);
    for i in 1..pts.len() - 1 {
        let a = pts[i].sub(p0);
        let b = pts[i + 1].sub(p0);
        let c = a.cross(b);
        sum = Vec3::new(sum.x + c.x, sum.y + c.y, sum.z + c.z);
    }
    0.5 * sum.dot(normal).abs()
}
