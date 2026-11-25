pub mod clamps;
pub mod config;
pub mod csv_edges;
pub mod detector;
pub mod lamello_p14;
pub mod model;
pub mod scx_writer;
pub mod step_parser;
pub mod transforms;

use std::fs;
use std::path::{Path, PathBuf};

use crate::detector::detect_panel_program;
use crate::model::{ConvertOptions, PanelProgram};
use crate::scx_writer::render_scx_xml;
use crate::step_parser::{NormalizedStepSolid, parse_and_normalize_step};
use crate::transforms::{PanelTransform, apply_panel_transform, push_simplified_transform};

/// Wczytany plik STEP gotowy do podglądu i wielokrotnego przeliczania po zmianie ustawień.
#[derive(Debug, Clone)]
pub struct LoadedStepPanel {
    pub source_path: Option<PathBuf>,
    pub file_stem: String,
    pub solid: Option<NormalizedStepSolid>,
    pub program: PanelProgram,
    pub edge_manual: [bool; 4],
    pub user_transforms: Vec<PanelTransform>,
    pub manual_skip: bool,
}

impl LoadedStepPanel {
    /// Ponownie uruchamia silnik detekcji na zapisanej bryle B-Rep z nowymi opcjami,
    /// zachowując ręczne pominięcie, obrzeża oraz wykonane przez użytkownika obroty/odbicia.
    pub fn redetect(&mut self, opts: &ConvertOptions) {
        if let Some(ref solid) = self.solid {
            let prev_edges = self.program.edges.clone();
            let prev_mat = self.program.material.clone();
            let prev_grain = self.program.grain;
            let mut next = detect_panel_program(solid, &self.file_stem, opts);
            for &xf in &self.user_transforms {
                apply_panel_transform(&mut next, None, xf, opts);
            }
            next.edges = prev_edges;
            if next.material.is_none() {
                next.material = prev_mat;
            }
            if next.grain == model::GrainDirection::None {
                next.grain = prev_grain;
            }
            self.program = next;
        }
    }

    /// Aplikuje obrót lub odbicie lustrzane na gotowych parametrach `PanelProgram` (natychmiastowo, wraz z obrzeżami).
    pub fn apply_transform(&mut self, xf: PanelTransform, opts: &ConvertOptions) {
        apply_panel_transform(&mut self.program, Some(&mut self.edge_manual), xf, opts);
        push_simplified_transform(&mut self.user_transforms, xf);
    }

    /// Cofa wszystkie ręczne obroty/odbicia do domyślnej orientacji wyznaczonej przez silnik (wraz z obrzeżami).
    pub fn reset_transforms(&mut self, opts: &ConvertOptions) {
        if self.user_transforms.is_empty() {
            return;
        }
        let inv_steps: Vec<PanelTransform> = self
            .user_transforms
            .iter()
            .rev()
            .map(|xf| xf.inverse())
            .collect();
        for inv in inv_steps {
            apply_panel_transform(&mut self.program, Some(&mut self.edge_manual), inv, opts);
        }
        self.user_transforms.clear();
    }
}

/// Wczytuje i analizuje pojedynczy plik `.step` / `.stp` bez zapisywania na dysk.
pub fn load_step_panel(
    input_path: &Path,
    opts: &ConvertOptions,
) -> Result<LoadedStepPanel, String> {
    let content = fs::read_to_string(input_path)
        .map_err(|e| format!("Błąd odczytu '{}': {}", input_path.display(), e))?;
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("panel")
        .to_string();

    let solid = parse_and_normalize_step(&content)?;
    let program = detect_panel_program(&solid, &stem, opts);

    Ok(LoadedStepPanel {
        source_path: Some(input_path.to_path_buf()),
        file_stem: stem,
        solid: Some(solid),
        program,
        edge_manual: [false; 4],
        user_transforms: Vec::new(),
        manual_skip: false,
    })
}

/// Zapisuje wygenerowany program `PanelProgram` do pliku `.scx` w `output_dir`.
/// Zwraca `Ok(None)`, jeśli formatka jest pusta i `skip_empty_panels == true`.
pub fn save_program_scx(
    program: &PanelProgram,
    file_stem: &str,
    output_dir: &Path,
    skip_empty_panels: bool,
) -> Result<Option<PathBuf>, String> {
    if skip_empty_panels && program.operations.is_empty() {
        return Ok(None);
    }
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Błąd tworzenia folderu '{}': {}", output_dir.display(), e))?;
    let out_path = output_dir.join(format!("{}.scx", file_stem));
    let xml = render_scx_xml(program);
    fs::write(&out_path, xml.as_bytes())
        .map_err(|e| format!("Błąd zapisu '{}': {}", out_path.display(), e))?;
    Ok(Some(out_path))
}

/// Jednoetapowa konwersja pliku `.step` → `.scx` (używana także przez CLI).
pub fn convert_step_file(
    input_path: &Path,
    output_dir: &Path,
    opts: &ConvertOptions,
) -> Result<Option<(PanelProgram, PathBuf)>, String> {
    let loaded = load_step_panel(input_path, opts)?;
    match save_program_scx(
        &loaded.program,
        &loaded.file_stem,
        output_dir,
        opts.skip_empty_panels,
    )? {
        Some(path) => Ok(Some((loaded.program, path))),
        None => Ok(None),
    }
}

pub fn format_num(v: f64) -> String {
    let clean = if v.abs() < 1e-6 { 0.0 } else { v };
    let s = format!("{:.4}", clean);
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::config::AppConfig;

    #[test]
    fn test_default_lamello_config_values() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.lamello_surface_width, 9.8);
        assert_eq!(cfg.lamello_surface_depth, 14.0);
        assert_eq!(cfg.lamello_edge_width, 10.15);
        assert_eq!(cfg.lamello_edge_depth, 14.0);
        assert_eq!(cfg.max_side_drill_depth, 50.0);
        assert_eq!(cfg.min_panel_width, 45.0);
        assert_eq!(cfg.anti_pinch_warning_width, 100.0);
    }

    #[test]
    fn test_ini_roundtrip() {
        let mut cfg = AppConfig::default();
        cfg.default_open_dir = r"C:\Projekty\Test\STEP".into();
        cfg.default_save_dir = r"C:\Projekty\Test\SCX".into();
        cfg.lamello_surface_depth = 14.1;
        let ini = cfg.to_ini();
        let parsed = AppConfig::from_ini(&ini);
        assert_eq!(parsed.default_open_dir, r"C:\Projekty\Test\STEP");
        assert_eq!(parsed.default_save_dir, r"C:\Projekty\Test\SCX");
        assert_eq!(parsed.lamello_surface_depth, 14.1);
    }

    #[test]
    fn test_short_id_extraction_any_order() {
        use crate::detector::extract_short_id;
        assert_eq!(extract_short_id("S01_03_18x364x400", None), "S01_03");
        assert_eq!(extract_short_id("14_Wieniec-polka_S14_P15", None), "S14_15");
        assert_eq!(extract_short_id("14_Wieniec-polka_S14_P16", None), "S14_16");
        assert_eq!(extract_short_id("02_Wieniec-polka_S12_P01", None), "S12_01");
        assert_eq!(extract_short_id("P08_S13_POZIOMA_DEKOR18", None), "S13_08");
        assert_eq!(
            extract_short_id("52_Listwa-pasa-skosnego-Minifix_Swspolne_P65", None),
            "SW_65"
        );
        assert_eq!(
            extract_short_id("custom_name", Some("P17_S14_SKOS_DEKOR18")),
            "S14_17"
        );
        assert_eq!(
            extract_short_id("formatka frez p14", None),
            "formatka frez p14"
        );
    }

    #[test]
    fn test_edge_lamello_tenso_vs_clamex_hole() {
        use crate::detector::detect_panel_program;
        use crate::model::{ConvertOptions, Operation, Point2, Vec3};
        use crate::step_parser::{NormalizedStepSolid, StepCircleEdge, StepCylinderFace};

        // 1. Tenso P-14: samo gniazdo krawędziowe R=50.2 na Y=0 (Face 2), bez otworu pionowego -> brak wiercenia
        let lam_cyl = StepCylinderFace {
            face_id: 1,
            origin: Vec3::new(150.0, -36.2, 4.0),
            axis: Vec3::new(0.0, 0.0, 1.0),
            radius: 50.2,
            vertices: vec![
                Vec3::new(115.0, 0.0, 4.0),
                Vec3::new(185.0, 0.0, 14.0),
                Vec3::new(150.0, 14.0, 9.0),
            ],
            circle_edges: vec![],
        };
        let solid_tenso = NormalizedStepSolid {
            product_name: None,
            length: 400.0,
            width: 300.0,
            thickness: 18.0,
            outline: vec![
                Point2::new(400.0, 300.0),
                Point2::new(0.0, 300.0),
                Point2::new(0.0, 0.0),
                Point2::new(400.0, 0.0),
            ],
            cylinders: vec![lam_cyl.clone()],
            planes: vec![],
        };
        let prog_tenso =
            detect_panel_program(&solid_tenso, "S01_03_tenso", &ConvertOptions::default());
        assert_eq!(prog_tenso.operations.len(), 1);
        // Auto-orientacja w osi Z obraca formatkę o 180°, aby gniazdo boczne było naprzeciw łap (Face 1 zamiast Face 2)!
        assert!(matches!(
            prog_tenso.operations[0],
            Operation::LamelloP14 { face: 1, .. }
        ));

        // 2. Clamex P-14: to samo gniazdo + płytki otwór Ø6 z góry w obrębie gniazda -> dodane wiercenie Face 5 do osi gniazda (9 mm)
        let clamex_hole = StepCylinderFace {
            face_id: 2,
            origin: Vec3::new(150.0, 7.5, 18.0),
            axis: Vec3::new(0.0, 0.0, 1.0),
            radius: 3.0,
            vertices: vec![Vec3::new(147.0, 7.5, 14.0), Vec3::new(153.0, 7.5, 18.0)],
            circle_edges: vec![StepCircleEdge {
                center: Vec3::new(150.0, 7.5, 18.0),
                axis: Vec3::new(0.0, 0.0, 1.0),
                radius: 3.0,
                v_start: Vec3::new(153.0, 7.5, 18.0),
                v_end: Vec3::new(153.0, 7.5, 18.0),
                is_full_circle: true,
                sweep_deg: 360.0,
            }],
        };
        let solid_clamex = NormalizedStepSolid {
            cylinders: vec![lam_cyl, clamex_hole],
            ..solid_tenso
        };
        let prog_clamex =
            detect_panel_program(&solid_clamex, "S01_04_clamex", &ConvertOptions::default());
        assert_eq!(prog_clamex.operations.len(), 2);
        let vdrill = prog_clamex
            .operations
            .iter()
            .find(|o| matches!(o, Operation::VerticalDrill { .. }))
            .expect("Clamex powinien zawierać wiercenie od góry");
        if let Operation::VerticalDrill {
            face,
            diameter,
            depth,
            ..
        } = *vdrill
        {
            assert_eq!(face, 5);
            assert!((diameter - 6.0).abs() < 1e-4);
            assert!((depth - 9.0).abs() < 1e-4);
        }
    }

    #[test]
    fn test_transforms_and_smart_auto_orient() {
        use crate::model::{ConvertOptions, EdgeBanding, GrainDirection, Operation, PanelProgram, Point2};
        use crate::transforms::{PanelTransform, apply_panel_transform, auto_orient_in_z};

        let opts = ConvertOptions::default();

        // 1. Przewrót do góry nogami: otwór przy łapach (Y = 20, Face 5) przechodzi na przeciwny bok (Y = 280) i jest wiercony od dołu (Face 6)
        let mut p = PanelProgram {
            id: "T1".into(),
            name: "T1".into(),
            length: 500.0,
            width: 300.0,
            thickness: 18.0,
            material: None,
            grain: GrainDirection::Length,
            outline: vec![
                Point2::new(500.0, 300.0),
                Point2::new(0.0, 300.0),
                Point2::new(0.0, 0.0),
                Point2::new(500.0, 0.0),
            ],
            operations: vec![Operation::VerticalDrill {
                face: 5,
                x: 100.0,
                y: 20.0,
                diameter: 8.0,
                depth: 12.0,
                is_through: false,
            }],
            edges: EdgeBanding {
                face1_bottom: 0.0,
                face2_top: 2.0,
                face3_left: 0.8,
                face4_right: 0.0,
            },
            warnings: vec![],
            flipped_top_bottom: false,
        };

        apply_panel_transform(&mut p, None, PanelTransform::FlipUpsideDown, &opts);
        if let Operation::VerticalDrill { face, x, y, .. } = p.operations[0] {
            assert_eq!(face, 6);
            assert!((x - 100.0).abs() < 1e-4);
            assert!((y - 280.0).abs() < 1e-4);
        } else {
            panic!("Oczekiwano VerticalDrill");
        }
        assert!((p.edges.face1_bottom - 2.0).abs() < 1e-4);

        // +90° i −90° wracają dokładnie do stanu wyjściowego
        let snap = p.clone();
        apply_panel_transform(&mut p, None, PanelTransform::Rotate90Plus, &opts);
        assert!((p.length - 300.0).abs() < 1e-4 && (p.width - 500.0).abs() < 1e-4);
        apply_panel_transform(&mut p, None, PanelTransform::Rotate90Minus, &opts);
        assert!((p.length - snap.length).abs() < 1e-4 && (p.width - snap.width).abs() < 1e-4);

        // 2. Formatka 500×120 z 3 otworami bocznymi od strony łap (Face 2) -> obraca się o 180°, nigdy bokiem 120 do łap!
        let mut narrow = PanelProgram {
            id: "N1".into(),
            name: "N1".into(),
            length: 500.0,
            width: 120.0,
            thickness: 18.0,
            material: None,
            grain: GrainDirection::None,
            outline: vec![
                Point2::new(500.0, 120.0),
                Point2::new(0.0, 120.0),
                Point2::new(0.0, 0.0),
                Point2::new(500.0, 0.0),
            ],
            operations: vec![Operation::HorizontalDrill {
                face: 2,
                x: 250.0,
                y: 0.0,
                z: 9.0,
                diameter: 8.0,
                depth: 30.0,
            }],
            edges: EdgeBanding::default(),
            warnings: vec![],
            flipped_top_bottom: false,
        };
        auto_orient_in_z(&mut narrow, &opts);
        assert!((narrow.length - 500.0).abs() < 1e-4);
        assert!((narrow.width - 120.0).abs() < 1e-4);
        assert!(matches!(narrow.operations[0], Operation::HorizontalDrill { face: 1, .. }));

        // 3. Formatka w kształcie L z wycięciem u góry po lewej (brak oparcia dla lewej łapy na Y=0):
        //    auto_orient_in_z obraca ją tak, aby pełny prosty bok 600 mm był u góry przy łapach (Y=0) i miał kąt prosty w (0,0)!
        let mut l_panel = PanelProgram {
            id: "L1".into(),
            name: "L1".into(),
            length: 600.0,
            width: 600.0,
            thickness: 18.0,
            material: None,
            grain: GrainDirection::None,
            outline: vec![
                Point2::new(0.0, 0.0),
                Point2::new(300.0, 0.0),
                Point2::new(300.0, 250.0),
                Point2::new(600.0, 250.0),
                Point2::new(600.0, 600.0),
                Point2::new(0.0, 600.0),
            ],
            operations: vec![],
            edges: EdgeBanding::default(),
            warnings: vec![],
            flipped_top_bottom: false,
        };
        auto_orient_in_z(&mut l_panel, &opts);
        // Po obrocie pełna krawędź 600 mm leży na Y=0 (od X=0 do X=600) i prawa krawędź 600 mm leży na X=0!
        let has_origin = l_panel
            .outline
            .iter()
            .any(|pt| pt.x.abs() < 0.5 && pt.y.abs() < 0.5);
        let has_top_left = l_panel
            .outline
            .iter()
            .any(|pt| (pt.x - 600.0).abs() < 0.5 && pt.y.abs() < 0.5);
        assert!(has_origin && has_top_left);
    }
}
