pub mod clamps;
pub mod config;
pub mod csv_edges;
pub mod detector;
pub mod model;
pub mod scx_writer;
pub mod step_parser;

use std::fs;
use std::path::{Path, PathBuf};

use crate::detector::detect_panel_program;
use crate::model::{ConvertOptions, PanelProgram};
use crate::scx_writer::render_scx_xml;
use crate::step_parser::{NormalizedStepSolid, parse_and_normalize_step};

/// Wczytany plik STEP gotowy do podglądu i wielokrotnego przeliczania po zmianie ustawień.
#[derive(Debug, Clone)]
pub struct LoadedStepPanel {
    pub source_path: Option<PathBuf>,
    pub file_stem: String,
    pub solid: Option<NormalizedStepSolid>,
    pub program: PanelProgram,
    pub edge_manual: [bool; 4],
}

impl LoadedStepPanel {
    /// Ponownie uruchamia silnik detekcji na zapisanej bryle B-Rep z nowymi opcjami, zachowując obrzeża.
    pub fn redetect(&mut self, opts: &ConvertOptions) {
        if let Some(ref solid) = self.solid {
            let prev_edges = self.program.edges.clone();
            let prev_mat = self.program.material.clone();
            let prev_grain = self.program.grain;
            let mut next = detect_panel_program(solid, &self.file_stem, opts);
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
}
