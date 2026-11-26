use crate::model::{GrainDirection, Operation, PanelProgram, Point2};

/// Tryb konturu zewnętrznego na warstwie rozkroju w pliku DXF (odpowiednik `TrybKonturuRozkroju` z makra bSolid).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxfContourMode {
    /// Zawsze prostokąt gabarytowy `[0..Length] × [0..Width]` na warstwie `ROZKROJ`;
    /// jeśli rzeczywisty obrys ze STEPa ma wycięcia / ścięte rogi / łuki, trafia na `FREZOWANIE`.
    RectFormat,
    /// Rzeczywisty obrys zewnętrzny wyznaczony z bryły STEP bezpośrednio na warstwie `ROZKROJ`.
    StepOutline,
}

impl DxfContourMode {
    pub fn as_ini_str(self) -> &'static str {
        match self {
            Self::RectFormat => "rect",
            Self::StepOutline => "step",
        }
    }

    pub fn from_ini_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "step" | "2" | "outline" => Self::StepOutline,
            _ => Self::RectFormat,
        }
    }
}

/// Konfiguracja generatora 2D DXF pod bSolid oraz druk naklejek orientacyjnych.
#[derive(Debug, Clone, PartialEq)]
pub struct DxfExportOptions {
    pub contour_mode: DxfContourMode,
    /// Ukrywa strzałkę usłojenia nawet wtedy, gdy formatka ma ustawione `GrainDirection::Length` / `Width`.
    pub hide_grain_arrow: bool,
    /// Rozdziela otwory wiercone (`<= 36 mm`) na `WIERCENIE` oraz większe otwory/wycięcia na `FREZOWANIE`.
    /// Gdy `false`, wszystko poza rozkrojem, bokiem i strzałką trafia na wspólną warstwę `layer_widmo`.
    pub split_drill_and_mill: bool,
    /// Tworzy osobne warstwy z głębokością dla nieprzelotowych operacji (`Wiercenie GW12`, `Frezowanie GF6`).
    pub depth_layers: bool,
    pub draw_drills: bool,
    pub draw_side_holes: bool,
    pub draw_milling: bool,
    pub include_bottom_ops: bool,
    /// Próg średnicy wiercenia (domyślnie `36.0 mm` jak w `StepDoDXF_bSolid.bas`).
    pub drill_max_dia_mm: f64,
    /// Skala strzałki dla formatek `>= 300 × 200 mm` (domyślnie `0.40`).
    pub arrow_scale_large: f64,
    /// Skala strzałki dla mniejszych formatek `< 300 × 200 mm` (domyślnie `0.85`).
    pub arrow_scale_small: f64,
    pub layer_rozkroj: String,
    pub layer_wiercenie: String,
    pub layer_frezowanie: String,
    pub layer_wiercenie_bok: String,
    pub layer_lamello_bok: String,
    pub layer_lamello_pow: String,
    pub layer_widmo: String,
    pub layer_strzalka: String,
}

impl Default for DxfExportOptions {
    fn default() -> Self {
        Self {
            contour_mode: DxfContourMode::RectFormat,
            hide_grain_arrow: false,
            split_drill_and_mill: true,
            depth_layers: false,
            draw_drills: true,
            draw_side_holes: true,
            draw_milling: true,
            include_bottom_ops: true,
            drill_max_dia_mm: 36.0,
            arrow_scale_large: 0.40,
            arrow_scale_small: 0.85,
            layer_rozkroj: "ROZKROJ".to_string(),
            layer_wiercenie: "WIERCENIE".to_string(),
            layer_frezowanie: "FREZOWANIE".to_string(),
            layer_wiercenie_bok: "WIERCENIE_BOK".to_string(),
            layer_lamello_bok: "LAMELLO_BOK".to_string(),
            layer_lamello_pow: "LAMELLO_POW".to_string(),
            layer_widmo: "INSETS D 1.0".to_string(),
            layer_strzalka: "STRZALKA".to_string(),
        }
    }
}

// Standardowe kolory AutoCAD Color Index (ACI) odpowiadające stałym AcamColor z makra Alphacam + dedykowane Lamello:
const ACI_RED: i32 = 1; // acamRED (ROZKROJ)
const ACI_YELLOW: i32 = 2; // acamYELLOW (FREZOWANIE / INSETS D 1.0)
const ACI_GREEN: i32 = 3; // acamGREEN (WIERCENIE_BOK)
const ACI_CYAN: i32 = 4; // acamCYAN (WIERCENIE)
const ACI_MAGENTA: i32 = 6; // acamMAGENTA (STRZALKA)
const ACI_LAMELLO_EDGE: i32 = 30; // pomarańczowy (LAMELLO_BOK — łuki w krawędziach)
const ACI_LAMELLO_FACE: i32 = 211; // jasny fiolet (LAMELLO_POW — gniazda "kości" w powierzchni)

#[derive(Debug, Clone, Copy)]
struct DxfVertex {
    x: f64,
    y: f64,
    bulge: f64,
}

impl DxfVertex {
    fn plain(x: f64, y: f64) -> Self {
        Self { x, y, bulge: 0.0 }
    }

    fn with_bulge(x: f64, y: f64, bulge: f64) -> Self {
        Self { x, y, bulge }
    }
}

#[derive(Debug, Clone)]
enum DxfEntity {
    ClosedPolyline {
        layer: String,
        linetype: Option<&'static str>,
        vertices: Vec<DxfVertex>,
    },
    Circle {
        layer: String,
        cx: f64,
        cy: f64,
        radius: f64,
    },
}

#[derive(Debug, Clone)]
struct DxfLayerDef {
    name: String,
    color_aci: i32,
}

fn fmt_dxf(v: f64) -> String {
    let clean = if v.abs() < 1e-6 { 0.0 } else { v };
    let s = format!("{:.4}", clean);
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-0" {
        "0.0".to_string()
    } else if !trimmed.contains('.') {
        format!("{trimmed}.0")
    } else {
        trimmed.to_string()
    }
}

/// Formatuje głębokość warstwy `GW` / `GF` identycznie jak `FormatujGlebokoscWarstwy` w VBA (`GW10`, `GW13.5`).
pub fn format_depth_suffix(depth: f64) -> String {
    let g = (depth.abs() * 10.0).round() / 10.0;
    if (g - g.round()).abs() < 0.05 {
        format!("{}", g.round() as i64)
    } else {
        format!("{:.1}", g)
    }
}

/// Konwertuje współrzędne `(x, y)` z układu `PanelProgram` (MachiningPoint="1": `(0,0)` w prawym górnym rogu,
/// `X` w lewo, `Y` w dół) na prawoskrętny układ kartezjański 2D DXF z `(0,0)` w lewym dolnym narożniku
/// i widokiem z góry na stronę `Face 5` (dokładnie jak w podglądzie CAD `viewport.rs`, bez odbicia lustrzanego).
#[inline]
fn to_dxf_xy(panel: &PanelProgram, x: f64, y: f64) -> (f64, f64) {
    (panel.length - x, panel.width - y)
}

fn ensure_layer(layers: &mut Vec<DxfLayerDef>, name: &str, color_aci: i32) {
    let clean = name.trim();
    if clean.is_empty() {
        return;
    }
    if !layers
        .iter()
        .any(|l| l.name.eq_ignore_ascii_case(clean))
    {
        layers.push(DxfLayerDef {
            name: clean.to_string(),
            color_aci,
        });
    }
}

/// Generuje kompletny plik ASCII DXF (standard R12 `AC1009`, identyczny z `SaveDxfFile ..., 4` w Alphacamie).
pub fn render_dxf(panel: &PanelProgram, opts: &DxfExportOptions) -> String {
    let l = panel.length.max(0.0);
    let w = panel.width.max(0.0);

    let lyr_roz = non_empty_or(&opts.layer_rozkroj, "ROZKROJ");
    let lyr_wierc_base = non_empty_or(&opts.layer_wiercenie, "WIERCENIE");
    let lyr_frez_base = non_empty_or(&opts.layer_frezowanie, "FREZOWANIE");
    let lyr_side = non_empty_or(&opts.layer_wiercenie_bok, "WIERCENIE_BOK");
    let lyr_lam_bok = non_empty_or(&opts.layer_lamello_bok, "LAMELLO_BOK");
    let lyr_lam_pow = non_empty_or(&opts.layer_lamello_pow, "LAMELLO_POW");
    let lyr_widmo = non_empty_or(&opts.layer_widmo, "INSETS D 1.0");
    let lyr_arrow = non_empty_or(&opts.layer_strzalka, "STRZALKA");

    let mut layers: Vec<DxfLayerDef> = Vec::new();
    let mut entities: Vec<DxfEntity> = Vec::new();

    // 1. Warstwa ROZKROJ oraz ewentualny niestandardowy obrys na FREZOWANIE
    ensure_layer(&mut layers, &lyr_roz, ACI_RED);

    let pure_rect = is_pure_bounding_rect(&panel.outline, l, w);
    let outline_dxf_pts: Vec<(f64, f64, Option<f64>)> = panel
        .outline
        .iter()
        .map(|pt| {
            let (dx, dy) = to_dxf_xy(panel, pt.x, pt.y);
            (dx, dy, pt.radius2)
        })
        .collect();

    let rect_verts = vec![
        DxfVertex::plain(0.0, 0.0),
        DxfVertex::plain(l, 0.0),
        DxfVertex::plain(l, w),
        DxfVertex::plain(0.0, w),
    ];

    // Przechowujemy także odcinki konturu zewnętrznego (w układzie DXF) do przyklejania otworów bocznych
    let contour_segments_dxf: Vec<((f64, f64), (f64, f64))>;

    match opts.contour_mode {
        DxfContourMode::RectFormat => {
            entities.push(DxfEntity::ClosedPolyline {
                layer: lyr_roz.clone(),
                linetype: None,
                vertices: rect_verts,
            });
            contour_segments_dxf = vec![
                ((0.0, 0.0), (l, 0.0)),
                ((l, 0.0), (l, w)),
                ((l, w), (0.0, w)),
                ((0.0, w), (0.0, 0.0)),
            ];

            if !pure_rect && outline_dxf_pts.len() >= 3 && opts.draw_milling {
                let target_frez = if opts.split_drill_and_mill {
                    ensure_layer(&mut layers, &lyr_frez_base, ACI_YELLOW);
                    lyr_frez_base.clone()
                } else {
                    ensure_layer(&mut layers, &lyr_widmo, ACI_YELLOW);
                    lyr_widmo.clone()
                };
                let verts = build_contour_vertices_with_fillets(&outline_dxf_pts);
                if verts.len() >= 3 {
                    entities.push(DxfEntity::ClosedPolyline {
                        layer: target_frez,
                        linetype: None,
                        vertices: verts,
                    });
                }
            }
        }
        DxfContourMode::StepOutline => {
            if !pure_rect && outline_dxf_pts.len() >= 3 {
                let verts = build_contour_vertices_with_fillets(&outline_dxf_pts);
                contour_segments_dxf = segments_from_plain_pts(&outline_dxf_pts);
                entities.push(DxfEntity::ClosedPolyline {
                    layer: lyr_roz.clone(),
                    linetype: None,
                    vertices: if verts.len() >= 3 { verts } else { rect_verts },
                });
            } else {
                contour_segments_dxf = vec![
                    ((0.0, 0.0), (l, 0.0)),
                    ((l, 0.0), (l, w)),
                    ((l, w), (0.0, w)),
                    ((0.0, w), (0.0, 0.0)),
                ];
                entities.push(DxfEntity::ClosedPolyline {
                    layer: lyr_roz.clone(),
                    linetype: None,
                    vertices: rect_verts,
                });
            }
        }
    }

    // 2. Operacje technologiczne z `PanelProgram`
    for op in &panel.operations {
        match *op {
            Operation::VerticalDrill {
                face,
                x,
                y,
                diameter,
                depth,
                is_through,
            } => {
                if face == 6 && !opts.include_bottom_ops {
                    continue;
                }
                if diameter <= 0.1 {
                    continue;
                }
                let is_drill = diameter <= (opts.drill_max_dia_mm + 0.05);
                if is_drill && !opts.draw_drills {
                    continue;
                }
                if !is_drill && !opts.draw_milling {
                    continue;
                }

                let (layer_name, aci) = if !opts.split_drill_and_mill {
                    (lyr_widmo.clone(), ACI_YELLOW)
                } else if is_drill {
                    if is_through || !opts.depth_layers || depth <= 0.05 {
                        (lyr_wierc_base.clone(), ACI_CYAN)
                    } else {
                        (
                            format!("Wiercenie GW{}", format_depth_suffix(depth)),
                            ACI_CYAN,
                        )
                    }
                } else if is_through || !opts.depth_layers || depth <= 0.05 {
                    (lyr_frez_base.clone(), ACI_YELLOW)
                } else {
                    (
                        format!("Frezowanie GF{}", format_depth_suffix(depth)),
                        ACI_YELLOW,
                    )
                };

                ensure_layer(&mut layers, &layer_name, aci);
                let (cx, cy) = to_dxf_xy(panel, x, y);
                entities.push(DxfEntity::Circle {
                    layer: layer_name,
                    cx,
                    cy,
                    radius: diameter * 0.5,
                });
            }

            Operation::HorizontalDrill {
                face,
                x,
                y,
                diameter,
                depth,
                ..
            } => {
                if !opts.draw_side_holes || diameter <= 0.1 || depth <= 0.2 {
                    continue;
                }
                ensure_layer(&mut layers, &lyr_side, ACI_GREEN);

                if let Some(verts) = build_side_hole_rect_dxf(
                    panel,
                    face,
                    x,
                    y,
                    diameter * 0.5,
                    depth,
                    &contour_segments_dxf,
                ) {
                    entities.push(DxfEntity::ClosedPolyline {
                        layer: lyr_side.clone(),
                        linetype: None,
                        vertices: verts,
                    });
                }
            }

            Operation::Groove {
                face,
                x,
                y,
                end_x,
                end_y,
                depth,
                width,
                ref tool_offset,
                ..
            } => {
                if face == 6 && !opts.include_bottom_ops {
                    continue;
                }
                if !opts.draw_milling || width <= 0.1 {
                    continue;
                }
                let is_thru = depth >= (panel.thickness - 0.15);
                let (layer_name, aci) = resolve_milling_layer(
                    opts,
                    &lyr_frez_base,
                    &lyr_widmo,
                    depth,
                    is_thru,
                );
                ensure_layer(&mut layers, &layer_name, aci);

                if let Some(verts) =
                    build_groove_rect_dxf(panel, x, y, end_x, end_y, width, tool_offset)
                {
                    entities.push(DxfEntity::ClosedPolyline {
                        layer: layer_name,
                        linetype: None,
                        vertices: verts,
                    });
                }
            }

            Operation::LamelloP14 {
                face,
                direction,
                x,
                y,
                max_depth,
                ..
            } => {
                if (1..=4).contains(&face) {
                    if !opts.draw_side_holes || max_depth <= 0.2 {
                        continue;
                    }
                    ensure_layer(&mut layers, &lyr_lam_bok, ACI_LAMELLO_EDGE);
                    if let Some(verts) = build_edge_lamello_arc_dxf(
                        panel,
                        face,
                        x,
                        y,
                        max_depth,
                        &contour_segments_dxf,
                    ) {
                        entities.push(DxfEntity::ClosedPolyline {
                            layer: lyr_lam_bok.clone(),
                            linetype: None,
                            vertices: verts,
                        });
                    }
                } else {
                    if face == 6 && !opts.include_bottom_ops {
                        continue;
                    }
                    if !opts.draw_milling {
                        continue;
                    }
                    ensure_layer(&mut layers, &lyr_lam_pow, ACI_LAMELLO_FACE);
                    let (cx, cy) = to_dxf_xy(panel, x, y);
                    let verts = build_face_lamello_bone_dxf(cx, cy, direction == 'X');
                    entities.push(DxfEntity::ClosedPolyline {
                        layer: lyr_lam_pow.clone(),
                        // Gniazda powierzchniowe ze spodu (Face 6) rysujemy linią przerywaną DASHED na tej samej warstwie LAMELLO_POW
                        linetype: if face == 6 { Some("DASHED") } else { None },
                        vertices: verts,
                    });
                }
            }
        }
    }

    // 3. Strzałka usłojenia na środku formatki (zgodnie z `panel.grain`)
    if !opts.hide_grain_arrow && panel.grain != GrainDirection::None && l > 1.0 && w > 1.0 {
        if let Some(arrow_verts) = build_grain_arrow_dxf(l, w, panel.grain, opts) {
            ensure_layer(&mut layers, &lyr_arrow, ACI_MAGENTA);
            entities.push(DxfEntity::ClosedPolyline {
                layer: lyr_arrow,
                linetype: None,
                vertices: arrow_verts,
            });
        }
    }

    serialize_r12_dxf(&layers, &entities, l, w)
}

fn resolve_milling_layer(
    opts: &DxfExportOptions,
    lyr_frez_base: &str,
    lyr_widmo: &str,
    depth: f64,
    is_through: bool,
) -> (String, i32) {
    if !opts.split_drill_and_mill {
        (lyr_widmo.to_string(), ACI_YELLOW)
    } else if is_through || !opts.depth_layers || depth <= 0.05 {
        (lyr_frez_base.to_string(), ACI_YELLOW)
    } else {
        (
            format!("Frezowanie GF{}", format_depth_suffix(depth)),
            ACI_YELLOW,
        )
    }
}

fn non_empty_or(val: &str, fallback: &str) -> String {
    let t = val.trim();
    if t.is_empty() {
        fallback.to_string()
    } else {
        t.to_string()
    }
}

/// Sprawdza, czy kontur z `PanelProgram` jest czystym prostokątem gabarytowym `[0..L] × [0..W]` bez łuków i ścięć.
pub fn is_pure_bounding_rect(outline: &[Point2], l: f64, w: f64) -> bool {
    if outline.len() != 4 {
        return false;
    }
    let tol = 0.5;
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for i in 0..4 {
        let p = outline[i];
        if p.radius2.unwrap_or(0.0).abs() > 0.05 {
            return false;
        }
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);

        let is_corner_x = p.x.abs() <= tol || (p.x - l).abs() <= tol;
        let is_corner_y = p.y.abs() <= tol || (p.y - w).abs() <= tol;
        if !is_corner_x || !is_corner_y {
            return false;
        }

        let next = outline[(i + 1) % 4];
        let horiz_or_vert = (p.x - next.x).abs() <= tol || (p.y - next.y).abs() <= tol;
        if !horiz_or_vert {
            return false;
        }
    }

    min_x.abs() <= tol
        && (max_x - l).abs() <= tol
        && min_y.abs() <= tol
        && (max_y - w).abs() <= tol
}

fn segments_from_plain_pts(pts: &[(f64, f64, Option<f64>)]) -> Vec<((f64, f64), (f64, f64))> {
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        out.push(((a.0, a.1), (b.0, b.1)));
    }
    out
}

/// Rozwija wierzchołki konturu z opcjonalnymi zaokrągleniami narożników `radius2` na wierzchołki polilinii DXF z `bulge`.
fn build_contour_vertices_with_fillets(pts: &[(f64, f64, Option<f64>)]) -> Vec<DxfVertex> {
    let n = pts.len();
    if n < 3 {
        return pts
            .iter()
            .map(|&(x, y, _)| DxfVertex::plain(x, y))
            .collect();
    }

    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let prev = pts[(i + n - 1) % n];
        let cur = pts[i];
        let next = pts[(i + 1) % n];

        let r = cur.2.unwrap_or(0.0).abs();
        if r <= 0.05 {
            out.push(DxfVertex::plain(cur.0, cur.1));
            continue;
        }

        let v_in = (prev.0 - cur.0, prev.1 - cur.1);
        let v_out = (next.0 - cur.0, next.1 - cur.1);
        let len_in = (v_in.0 * v_in.0 + v_in.1 * v_in.1).sqrt();
        let len_out = (v_out.0 * v_out.0 + v_out.1 * v_out.1).sqrt();

        if len_in < 1e-4 || len_out < 1e-4 {
            out.push(DxfVertex::plain(cur.0, cur.1));
            continue;
        }

        let u_in = (v_in.0 / len_in, v_in.1 / len_in);
        let u_out = (v_out.0 / len_out, v_out.1 / len_out);

        // Kąt wewnętrzny między u_in a u_out:
        let dot = (u_in.0 * u_out.0 + u_in.1 * u_out.1).clamp(-1.0, 1.0);
        let alpha = dot.acos(); // (0..PI)
        if alpha < 0.05 || (std::f64::consts::PI - alpha) < 0.05 {
            out.push(DxfVertex::plain(cur.0, cur.1));
            continue;
        }

        let half_alpha = alpha * 0.5;
        let max_tan = len_in.min(len_out) * 0.49;
        let tan_dist = (r / half_alpha.tan()).min(max_tan);
        if tan_dist <= 0.02 {
            out.push(DxfVertex::plain(cur.0, cur.1));
            continue;
        }

        let p_start = (cur.0 + u_in.0 * tan_dist, cur.1 + u_in.1 * tan_dist);
        let p_end = (cur.0 + u_out.0 * tan_dist, cur.1 + u_out.1 * tan_dist);

        // Kąt środkowy łuku = PI - alpha. Kierunek skrętu od wektora (-u_in) do u_out:
        let dir_before = (-u_in.0, -u_in.1);
        let cross = dir_before.0 * u_out.1 - dir_before.1 * u_out.0;
        let sweep = std::f64::consts::PI - alpha;
        let sign = if cross >= 0.0 { 1.0 } else { -1.0 };
        let bulge = sign * (sweep * 0.25).tan();

        out.push(DxfVertex {
            x: p_start.0,
            y: p_start.1,
            bulge,
        });
        out.push(DxfVertex::plain(p_end.0, p_end.1));
    }

    out
}

/// Buduje prostokąt otworu bocznego (`WIERCENIE_BOK`) przyklejony jednym bokiem do krawędzi konturu rozkroju
/// (dokładny odpowiednik `WyciagnijOtworyWKrawedziach` + `PrzyklejBokOtworuDoKonturu` z `StepDoDXF_bSolid.bas`).
fn build_side_hole_rect_dxf(
    panel: &PanelProgram,
    face: u8,
    x: f64,
    y: f64,
    half_width: f64,
    depth: f64,
    contour_segments_dxf: &[((f64, f64), (f64, f64))],
) -> Option<Vec<DxfVertex>> {
    // Punkt wejścia i punkt dna w układzie `PanelProgram`, następnie konwersja do DXF:
    let (p_entry_pp, p_inner_pp) = match face {
        4 => ((x, y), (x + depth, y)), // Prawa krawędź DXF (X_dxf = L -> L - depth)
        3 => ((x, y), (x - depth, y)), // Lewa krawędź DXF (X_dxf = 0 -> depth)
        2 => ((x, y), (x, y + depth)), // Górna krawędź DXF (Y_dxf = W -> W - depth)
        1 => ((x, y), (x, y - depth)), // Dolna krawędź DXF (Y_dxf = 0 -> depth)
        _ => return None,
    };

    let (mut x1, mut y1) = to_dxf_xy(panel, p_entry_pp.0, p_entry_pp.1);
    let (x2, y2) = to_dxf_xy(panel, p_inner_pp.0, p_inner_pp.1);

    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 0.2 {
        return None;
    }
    let ux = dx / len;
    let uy = dy / len;

    // Przyklejenie boku wejściowego `(x1, y1)` do najbliższego przecięcia osi z konturem rozkroju (maks. 25 mm):
    const MAX_SNAP_DIST: f64 = 25.0;
    let mut best_dist = f64::INFINITY;
    let mut best_t = 0.0_f64;

    for &(seg_a, seg_b) in contour_segments_dxf {
        if let Some(t_hit) = intersect_ray_line_with_segment(x1, y1, ux, uy, seg_a, seg_b) {
            let d = t_hit.abs();
            // Po przyklejeniu długość otworu `(len - t_hit)` musi pozostać dodatnia (> 0.5 mm)
            if d < best_dist && d <= MAX_SNAP_DIST && (len - t_hit) > 0.5 {
                best_dist = d;
                best_t = t_hit;
            }
        }
    }

    if best_dist <= MAX_SNAP_DIST {
        x1 += best_t * ux;
        y1 += best_t * uy;
    }

    let nx = -uy;
    let ny = ux;

    Some(vec![
        DxfVertex::plain(x1 - half_width * nx, y1 - half_width * ny),
        DxfVertex::plain(x2 - half_width * nx, y2 - half_width * ny),
        DxfVertex::plain(x2 + half_width * nx, y2 + half_width * ny),
        DxfVertex::plain(x1 + half_width * nx, y1 + half_width * ny),
    ])
}

/// Buduje łukowate gniazdo krawędziowe Lamello P14 widoczne od góry (odcinek koła R=50.2 mm + prosta cięciwa na krawędzi).
fn build_edge_lamello_arc_dxf(
    panel: &PanelProgram,
    face: u8,
    x: f64,
    y: f64,
    max_depth: f64,
    contour_segments_dxf: &[((f64, f64), (f64, f64))],
) -> Option<Vec<DxfVertex>> {
    const R: f64 = 50.2;
    let h = max_depth.clamp(1.0, R * 0.92);
    let th = ((R - h) / R).acos();
    let half_chord = R * th.sin();
    let bulge = (th * 0.5).tan();

    let (p_entry_pp, p_inner_pp) = match face {
        4 => ((x, y), (x + h, y)), // Prawa krawędź DXF -> w głąb w lewo (-X)
        3 => ((x, y), (x - h, y)), // Lewa krawędź DXF -> w głąb w prawo (+X)
        2 => ((x, y), (x, y + h)), // Górna krawędź DXF -> w głąb w dół (-Y)
        1 => ((x, y), (x, y - h)), // Dolna krawędź DXF -> w głąb w górę (+Y)
        _ => return None,
    };

    let (mut mx, mut my) = to_dxf_xy(panel, p_entry_pp.0, p_entry_pp.1);
    let (ix_pt, iy_pt) = to_dxf_xy(panel, p_inner_pp.0, p_inner_pp.1);
    let dx = ix_pt - mx;
    let dy = iy_pt - my;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 0.2 {
        return None;
    }
    // Wektor jednostkowy `I = (ix, iy)` skierowany do wewnątrz formatki
    let ix = dx / len;
    let iy = dy / len;

    // Przyklejenie środka cięciwy do krawędzi konturu rozkroju (maks. 25 mm):
    const MAX_SNAP_DIST: f64 = 25.0;
    let mut best_dist = f64::INFINITY;
    let mut best_t = 0.0_f64;
    for &(seg_a, seg_b) in contour_segments_dxf {
        if let Some(t_hit) = intersect_ray_line_with_segment(mx, my, ix, iy, seg_a, seg_b) {
            let d = t_hit.abs();
            if d < best_dist && d <= MAX_SNAP_DIST && (h - t_hit) > 1.0 {
                best_dist = d;
                best_t = t_hit;
            }
        }
    }
    if best_dist <= MAX_SNAP_DIST {
        mx += best_t * ix;
        my += best_t * iy;
    }

    // Wektor styczny `T = (iy, -ix)` wzdłuż krawędzi:
    // Od P1 = M + half_chord * T do P2 = M - half_chord * T łuk CCW (`bulge > 0`) wygina się dokładnie w stronę `M + h * I`!
    let tx = iy;
    let ty = -ix;

    let p1x = mx + half_chord * tx;
    let p1y = my + half_chord * ty;
    let p2x = mx - half_chord * tx;
    let p2y = my - half_chord * ty;

    Some(vec![
        DxfVertex::with_bulge(p1x, p1y, bulge),
        DxfVertex::plain(p2x, p2y),
    ])
}

/// Buduje uproszczone gniazdo powierzchniowe Lamello P14 w kształcie „kości” (wąski środek 7 mm + pogrubione półokrągłe końce Ø10 mm).
fn build_face_lamello_bone_dxf(cx: f64, cy: f64, axis_along_x: bool) -> Vec<DxfVertex> {
    let p14 = crate::lamello_p14::P14::default();
    let l = crate::lamello_p14::chord(p14.cutter_d * 0.5, p14.face_depth);
    let h = l * 0.5;
    let nw = p14.sock_w * 0.5; // 3.5 mm
    let e2 = p14.end_w * 0.5; // 5.0 mm
    let el = p14.end_len; // 7.0 mm
    let r = p14.end_r.min(el).min(e2); // 5.0 mm

    // 12 wierzchołków CCW w układzie lokalnym `(a, b)` z dwoma idealnymi półkolami `bulge = +1.0` na końcach:
    let local: [(f64, f64, f64); 12] = [
        (-h + el, -nw, 0.0),
        (h - el, -nw, 0.0),
        (h - el, -e2, 0.0),
        (h - r, -e2, 1.0), // półkole prawego końca przez (h, 0)
        (h - r, e2, 0.0),
        (h - el, e2, 0.0),
        (h - el, nw, 0.0),
        (-h + el, nw, 0.0),
        (-h + el, e2, 0.0),
        (-h + r, e2, 1.0), // półkole lewego końca przez (-h, 0)
        (-h + r, -e2, 0.0),
        (-h + el, -e2, 0.0),
    ];

    local
        .into_iter()
        .map(|(a, b, bulge)| {
            // Obie macierze mają wyznacznik +1, więc znak `bulge` (CCW) zostaje zachowany bez odwrócenia!
            let (dx, dy) = if axis_along_x {
                (cx + a, cy + b)
            } else {
                (cx - b, cy + a)
            };
            DxfVertex { x: dx, y: dy, bulge }
        })
        .collect()
}

fn intersect_ray_line_with_segment(
    ax: f64,
    ay: f64,
    ux: f64,
    uy: f64,
    seg_a: (f64, f64),
    seg_b: (f64, f64),
) -> Option<f64> {
    let sx = seg_b.0 - seg_a.0;
    let sy = seg_b.1 - seg_a.1;
    let det = ux * (-sy) - uy * (-sx);
    if det.abs() < 1e-6 {
        return None;
    }
    let rx = seg_a.0 - ax;
    let ry = seg_a.1 - ay;
    let t = (rx * (-sy) - ry * (-sx)) / det;
    let u = (ux * ry - uy * rx) / det;
    if (-0.001..=1.001).contains(&u) {
        Some(t)
    } else {
        None
    }
}

fn build_groove_rect_dxf(
    panel: &PanelProgram,
    x: f64,
    y: f64,
    end_x: f64,
    end_y: f64,
    width: f64,
    tool_offset: &str,
) -> Option<Vec<DxfVertex>> {
    // Wyznaczamy lewą i prawą krawędź rowka najpierw w układzie `PanelProgram`, a potem mapujemy przez `to_dxf_xy`.
    let dx = end_x - x;
    let dy = end_y - y;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 0.1 {
        return None;
    }
    let ux = dx / len;
    let uy = dy / len;
    // Wektor normalny w lewo względem kierunku ruchu w układzie `PanelProgram`:
    let nx = -uy;
    let ny = ux;

    let (off_min, off_max) = match tool_offset.trim() {
        "左" | "L" | "Left" => (0.0, width),
        "右" | "R" | "Right" => (-width, 0.0),
        _ => (-width * 0.5, width * 0.5),
    };

    let corners_pp = [
        (x + nx * off_min, y + ny * off_min),
        (end_x + nx * off_min, end_y + ny * off_min),
        (end_x + nx * off_max, end_y + ny * off_max),
        (x + nx * off_max, y + ny * off_max),
    ];

    Some(
        corners_pp
            .into_iter()
            .map(|(px, py)| {
                let (dx_cad, dy_cad) = to_dxf_xy(panel, px, py);
                DxfVertex::plain(dx_cad, dy_cad)
            })
            .collect(),
    )
}

/// Rysuje symetryczną strzałkę usłojenia o stałej proporcji `1.5 : 1` na środku formatki
/// (adaptacja `NarysujStrzalkeNaSrodku` z obsługą `GrainDirection::Length` -> `+X` oraz `GrainDirection::Width` -> `+Y`).
fn build_grain_arrow_dxf(
    d_x: f64,
    d_y: f64,
    grain: GrainDirection,
    opts: &DxfExportOptions,
) -> Option<Vec<DxfVertex>> {
    if grain == GrainDirection::None || d_x <= 1.0 || d_y <= 1.0 {
        return None;
    }

    let cx = d_x * 0.5;
    let cy = d_y * 0.5;
    let margin = 2.0_f64;

    let scale_large = if opts.arrow_scale_large > 0.0 && opts.arrow_scale_large < 1.0 {
        opts.arrow_scale_large
    } else {
        0.40
    };
    let scale_small = if opts.arrow_scale_small > 0.0 && opts.arrow_scale_small < 1.0 {
        opts.arrow_scale_small
    } else {
        0.85
    };

    let smaller_dim = d_x.min(d_y);
    let max_dim = d_x.max(d_y);
    let scale = if max_dim < 300.0 || smaller_dim < 200.0 {
        scale_small
    } else {
        scale_large
    };

    const RATIO_LEN_TO_WIDTH: f64 = 1.5;

    // Wymiar wzdłużny (`along_dim`) i poprzeczny (`cross_dim`) zależnie od kierunku usłojenia:
    let (along_dim, cross_dim) = match grain {
        GrainDirection::Length => (d_x, d_y),
        GrainDirection::Width => (d_y, d_x),
        GrainDirection::None => return None,
    };

    let mut h_head = smaller_dim * scale;
    let mut arrow_len = h_head * RATIO_LEN_TO_WIDTH;

    let mut max_allowed_len = along_dim * scale;
    if max_allowed_len > (along_dim - 2.0 * margin) {
        max_allowed_len = (along_dim - 2.0 * margin).max(1.0);
    }
    if arrow_len > max_allowed_len {
        arrow_len = max_allowed_len;
        h_head = arrow_len / RATIO_LEN_TO_WIDTH;
    }

    let max_allowed_h = (cross_dim - 2.0 * margin).max(1.0);
    if h_head > max_allowed_h {
        h_head = max_allowed_h;
        arrow_len = h_head * RATIO_LEN_TO_WIDTH;
    }

    if h_head < 2.0 {
        h_head = (smaller_dim * 0.6).max(0.8);
        arrow_len = (h_head * RATIO_LEN_TO_WIDTH).min((along_dim - 1.0).max(1.0));
    }

    let h_shaft = h_head * 0.38;
    let head_len = arrow_len * 0.34;

    let a_start = -arrow_len * 0.5;
    let a_end = arrow_len * 0.5;
    let a_neck = a_end - head_len;

    let b_shaft = h_shaft * 0.5;
    let b_head = h_head * 0.5;

    // 7 unikalnych wierzchołków w układzie lokalnym `(a, b)`, gdzie `a` to kierunek grotu:
    let local_pts = [
        (a_start, -b_shaft),
        (a_neck, -b_shaft),
        (a_neck, -b_head),
        (a_end, 0.0),
        (a_neck, b_head),
        (a_neck, b_shaft),
        (a_start, b_shaft),
    ];

    let verts = local_pts
        .into_iter()
        .map(|(a, b)| match grain {
            GrainDirection::Length => DxfVertex::plain(cx + a, cy + b),
            GrainDirection::Width => DxfVertex::plain(cx - b, cy + a),
            GrainDirection::None => unreachable!(),
        })
        .collect();

    Some(verts)
}

fn serialize_r12_dxf(
    layers: &[DxfLayerDef],
    entities: &[DxfEntity],
    max_x: f64,
    max_y: f64,
) -> String {
    let mut out = String::with_capacity(4096 + entities.len() * 160);

    // ── HEADER ──
    out.push_str("0\r\nSECTION\r\n2\r\nHEADER\r\n");
    out.push_str("9\r\n$ACADVER\r\n1\r\nAC1009\r\n");
    out.push_str(&format!(
        "9\r\n$EXTMIN\r\n10\r\n0.0\r\n20\r\n0.0\r\n30\r\n0.0\r\n\
         9\r\n$EXTMAX\r\n10\r\n{}\r\n20\r\n{}\r\n30\r\n0.0\r\n",
        fmt_dxf(max_x),
        fmt_dxf(max_y)
    ));
    out.push_str("0\r\nENDSEC\r\n");

    // ── TABLES (LTYPE + LAYER) ──
    out.push_str("0\r\nSECTION\r\n2\r\nTABLES\r\n");

    out.push_str("0\r\nTABLE\r\n2\r\nLTYPE\r\n70\r\n2\r\n");
    out.push_str(
        "0\r\nLTYPE\r\n2\r\nCONTINUOUS\r\n70\r\n0\r\n3\r\nSolid line\r\n72\r\n65\r\n73\r\n0\r\n40\r\n0.0\r\n",
    );
    out.push_str(
        "0\r\nLTYPE\r\n2\r\nDASHED\r\n70\r\n0\r\n3\r\nDashed __ __ __\r\n72\r\n65\r\n73\r\n2\r\n40\r\n6.5\r\n49\r\n4.0\r\n49\r\n-2.5\r\n",
    );
    out.push_str("0\r\nENDTAB\r\n");

    out.push_str(&format!(
        "0\r\nTABLE\r\n2\r\nLAYER\r\n70\r\n{}\r\n",
        layers.len() + 1
    ));
    out.push_str("0\r\nLAYER\r\n2\r\n0\r\n70\r\n0\r\n62\r\n7\r\n6\r\nCONTINUOUS\r\n");
    for lyr in layers {
        out.push_str(&format!(
            "0\r\nLAYER\r\n2\r\n{}\r\n70\r\n0\r\n62\r\n{}\r\n6\r\nCONTINUOUS\r\n",
            lyr.name, lyr.color_aci
        ));
    }
    out.push_str("0\r\nENDTAB\r\n");
    out.push_str("0\r\nENDSEC\r\n");

    // ── ENTITIES ──
    out.push_str("0\r\nSECTION\r\n2\r\nENTITIES\r\n");
    for ent in entities {
        match ent {
            DxfEntity::Circle {
                layer,
                cx,
                cy,
                radius,
            } => {
                out.push_str(&format!(
                    "0\r\nCIRCLE\r\n8\r\n{}\r\n10\r\n{}\r\n20\r\n{}\r\n30\r\n0.0\r\n40\r\n{}\r\n",
                    layer,
                    fmt_dxf(*cx),
                    fmt_dxf(*cy),
                    fmt_dxf(*radius)
                ));
            }
            DxfEntity::ClosedPolyline {
                layer,
                linetype,
                vertices,
            } => {
                if vertices.len() < 2 {
                    continue;
                }
                if let Some(lt) = linetype {
                    out.push_str(&format!(
                        "0\r\nPOLYLINE\r\n8\r\n{}\r\n6\r\n{}\r\n66\r\n1\r\n10\r\n0.0\r\n20\r\n0.0\r\n30\r\n0.0\r\n70\r\n1\r\n",
                        layer, lt
                    ));
                } else {
                    out.push_str(&format!(
                        "0\r\nPOLYLINE\r\n8\r\n{}\r\n66\r\n1\r\n10\r\n0.0\r\n20\r\n0.0\r\n30\r\n0.0\r\n70\r\n1\r\n",
                        layer
                    ));
                }
                for v in vertices {
                    out.push_str(&format!(
                        "0\r\nVERTEX\r\n8\r\n{}\r\n10\r\n{}\r\n20\r\n{}\r\n30\r\n0.0\r\n",
                        layer,
                        fmt_dxf(v.x),
                        fmt_dxf(v.y)
                    ));
                    if v.bulge.abs() > 1e-6 {
                        out.push_str(&format!("42\r\n{}\r\n", fmt_dxf(v.bulge)));
                    }
                }
                out.push_str(&format!("0\r\nSEQEND\r\n8\r\n{}\r\n", layer));
            }
        }
    }
    out.push_str("0\r\nENDSEC\r\n0\r\nEOF\r\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EdgeBanding;

    fn sample_panel() -> PanelProgram {
        PanelProgram {
            id: "S01_01".into(),
            name: "Bok_P01".into(),
            length: 600.0,
            width: 400.0,
            thickness: 18.0,
            material: Some("EGGER_W1000".into()),
            grain: GrainDirection::Length,
            outline: vec![
                Point2::new(600.0, 400.0),
                Point2::new(0.0, 400.0),
                Point2::new(0.0, 0.0),
                Point2::new(600.0, 0.0),
            ],
            operations: vec![
                // Wiercenie <= 36 mm nieprzelotowe (d=12)
                Operation::VerticalDrill {
                    face: 5,
                    x: 100.0,
                    y: 50.0,
                    diameter: 8.0,
                    depth: 12.0,
                    is_through: false,
                },
                // Duży otwór > 36 mm (Ø40 przelotowy -> FREZOWANIE)
                Operation::VerticalDrill {
                    face: 5,
                    x: 300.0,
                    y: 200.0,
                    diameter: 40.0,
                    depth: 18.0,
                    is_through: true,
                },
                // Otwór boczny na prawym boku (Face 4: x=0 w PanelProgram -> X_dxf = 600)
                Operation::HorizontalDrill {
                    face: 4,
                    x: 0.0,
                    y: 100.0,
                    z: 9.0,
                    diameter: 8.0,
                    depth: 30.0,
                },
                // Nuta wzdłuż X
                Operation::Groove {
                    face: 5,
                    x: 0.0,
                    y: 20.0,
                    z: 18.0,
                    end_x: 600.0,
                    end_y: 20.0,
                    end_z: 18.0,
                    depth: 6.5,
                    width: 4.0,
                    tool_offset: "中".into(),
                },
            ],
            edges: EdgeBanding::default(),
            warnings: vec![],
            flipped_top_bottom: false,
        }
    }

    #[test]
    fn test_bsolid_layers_and_coordinates() {
        let panel = sample_panel();
        let opts = DxfExportOptions::default();
        let dxf = render_dxf(&panel, &opts);

        assert!(dxf.contains("AC1009"));
        assert!(dxf.contains("\r\nROZKROJ\r\n"));
        assert!(dxf.contains("\r\nWIERCENIE\r\n"));
        assert!(dxf.contains("\r\nFREZOWANIE\r\n"));
        assert!(dxf.contains("\r\nWIERCENIE_BOK\r\n"));
        assert!(dxf.contains("\r\nSTRZALKA\r\n"));

        // Otwór pionowy (x=100, y=50) w PanelProgram -> w DXF (600 - 100 = 500, 400 - 50 = 350), promień 4.0
        assert!(
            dxf.contains("0\r\nCIRCLE\r\n8\r\nWIERCENIE\r\n10\r\n500.0\r\n20\r\n350.0\r\n30\r\n0.0\r\n40\r\n4.0\r\n")
        );
        // Otwór Ø40 -> na warstwie FREZOWANIE, promień 20.0
        assert!(
            dxf.contains("0\r\nCIRCLE\r\n8\r\nFREZOWANIE\r\n10\r\n300.0\r\n20\r\n200.0\r\n30\r\n0.0\r\n40\r\n20.0\r\n")
        );
    }

    #[test]
    fn test_depth_layers_gw_gf() {
        let panel = sample_panel();
        let mut opts = DxfExportOptions::default();
        opts.depth_layers = true;
        let dxf = render_dxf(&panel, &opts);

        assert!(dxf.contains("\r\nWiercenie GW12\r\n"));
        assert!(dxf.contains("\r\nFrezowanie GF6.5\r\n"));
    }

    #[test]
    fn test_grain_arrow_directions_and_none() {
        let mut panel = sample_panel();
        let opts = DxfExportOptions::default();

        // 1. GrainDirection::Length -> strzałka skierowana w +X: L = min(240, 600*0.4) = 240 -> grot w (300 + 120 = 420, 200)
        let dxf_l = render_dxf(&panel, &opts);
        assert!(dxf_l.contains("0\r\nVERTEX\r\n8\r\nSTRZALKA\r\n10\r\n420.0\r\n20\r\n200.0\r\n"));

        // 2. Brak usłojenia -> brak warstwy STRZALKA
        panel.grain = GrainDirection::None;
        let dxf_none = render_dxf(&panel, &opts);
        assert!(!dxf_none.contains("\r\nSTRZALKA\r\n"));

        // 3. GrainDirection::Width -> strzałka skierowana w +Y: max_allowed_len = 400*0.4 = 160 -> grot w (300, 200 + 80 = 280)
        panel.grain = GrainDirection::Width;
        let dxf_w = render_dxf(&panel, &opts);
        assert!(dxf_w.contains("\r\nSTRZALKA\r\n"));
        assert!(dxf_w.contains("0\r\nVERTEX\r\n8\r\nSTRZALKA\r\n10\r\n300.0\r\n20\r\n280.0\r\n"));
    }

    #[test]
    fn test_non_rect_outline_split_in_rect_format_mode() {
        let mut panel = sample_panel();
        panel.operations.clear();
        // Kształt L: 6 wierzchołków
        panel.outline = vec![
            Point2::new(0.0, 0.0),
            Point2::new(300.0, 0.0),
            Point2::new(300.0, 200.0),
            Point2::new(600.0, 200.0),
            Point2::new(600.0, 400.0),
            Point2::new(0.0, 400.0),
        ];

        let opts_rect = DxfExportOptions {
            contour_mode: DxfContourMode::RectFormat,
            ..DxfExportOptions::default()
        };
        let dxf_rect = render_dxf(&panel, &opts_rect);
        // W trybie RectFormat gabaryt jest na ROZKROJ, a kształt L trafia na FREZOWANIE:
        assert!(dxf_rect.contains("0\r\nPOLYLINE\r\n8\r\nROZKROJ\r\n"));
        assert!(dxf_rect.contains("0\r\nPOLYLINE\r\n8\r\nFREZOWANIE\r\n"));

        let opts_step = DxfExportOptions {
            contour_mode: DxfContourMode::StepOutline,
            ..DxfExportOptions::default()
        };
        let dxf_step = render_dxf(&panel, &opts_step);
        assert!(dxf_step.contains("0\r\nPOLYLINE\r\n8\r\nROZKROJ\r\n"));
        assert!(!dxf_step.contains("0\r\nPOLYLINE\r\n8\r\nFREZOWANIE\r\n"));
    }

    #[test]
    fn test_lamello_bok_arc_and_lamello_pow_bone_layers() {
        let mut panel = sample_panel();
        panel.operations = vec![
            // Gniazdo w krawędzi lewej (Face 3: x=600 w PanelProgram -> X_dxf = 0, w głąb +X do 14 mm)
            Operation::LamelloP14 {
                face: 3,
                direction: 'Y',
                x: 600.0,
                y: 200.0,
                z: 9.0,
                length: 66.0,
                width: 10.15,
                max_depth: 14.0,
            },
            // Gniazdo powierzchniowe na górze (Face 5)
            Operation::LamelloP14 {
                face: 5,
                direction: 'Y',
                x: 300.0,
                y: 200.0,
                z: 18.0,
                length: 66.0,
                width: 9.8,
                max_depth: 14.0,
            },
            // Gniazdo powierzchniowe na dole (Face 6 -> przerywane DASHED na LAMELLO_POW)
            Operation::LamelloP14 {
                face: 6,
                direction: 'Y',
                x: 100.0,
                y: 200.0,
                z: 0.0,
                length: 66.0,
                width: 9.8,
                max_depth: 14.0,
            },
        ];

        let opts = DxfExportOptions::default();
        let dxf = render_dxf(&panel, &opts);

        assert!(dxf.contains("\r\nLAMELLO_BOK\r\n"));
        assert!(dxf.contains("\r\nLAMELLO_POW\r\n"));
        // Łuk gniazda krawędziowego ma bulge (kod 42)
        assert!(dxf.contains("0\r\nPOLYLINE\r\n8\r\nLAMELLO_BOK\r\n"));
        assert!(dxf.contains("\r\n42\r\n"));
        // Kształt kości ma półkola z bulge = 1.0, a dolne gniazdo ma LTYPE = DASHED
        assert!(dxf.contains("0\r\nPOLYLINE\r\n8\r\nLAMELLO_POW\r\n66\r\n1\r\n"));
        assert!(dxf.contains("0\r\nPOLYLINE\r\n8\r\nLAMELLO_POW\r\n6\r\nDASHED\r\n"));
        assert!(dxf.contains("42\r\n1.0\r\n"));

        // Sprawdźmy matematyczny środek łuku dla krawędzi Face 3 (X_dxf = 0, Y_dxf = 200, w głąb +X = 14 mm):
        let verts = build_edge_lamello_arc_dxf(&panel, 3, 600.0, 200.0, 14.0, &[((0.0, 0.0), (0.0, 400.0))]).unwrap();
        assert_eq!(verts.len(), 2);
        let p1 = verts[0];
        let p2 = verts[1];
        assert!(p1.x.abs() < 1e-6 && p2.x.abs() < 1e-6);
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        // Środek łuku CCW w DXF: M + (bulge/2) * (dy, -dx)
        let arc_mid_x = 0.5 * (p1.x + p2.x) + 0.5 * p1.bulge * dy;
        let arc_mid_y = 0.5 * (p1.y + p2.y) + 0.5 * p1.bulge * (-dx);
        assert!((arc_mid_x - 14.0).abs() < 1e-6);
        assert!((arc_mid_y - 200.0).abs() < 1e-6);
    }
}
