use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use nanxing_core::config::AppConfig;
use nanxing_core::dxf_writer::DxfContourMode;
use nanxing_core::model::GrainDirection;
use nanxing_core::{
    load_step_panel, resolve_export_dirs, save_program_dxf, save_program_scx_passes,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliExportMode {
    Both,
    ScxOnly,
    DxfOnly,
}

fn print_usage() {
    println!("NanxingConvertert — STEP -> SCX + DXF (Nanxing CNC & bSolid)");
    println!("Użycie:");
    println!("  nanxing_cli <plik.step | folder> [opcje]");
    println!();
    println!("Opcje eksportu (zawsze do podfolderów SCX oraz DXF):");
    println!("  --both              Eksportuje jednocześnie SCX (do SCX\\) i DXF (do DXF\\) [domyślnie]");
    println!("  --scx-only          Eksportuje wyłącznie programy .scx (do podfolderu SCX\\)");
    println!("  --dxf-only          Eksportuje wyłącznie rysunki 2D .dxf (do podfolderu DXF\\)");
    println!("  --dxf               Włącza generowanie DXF (równoważne --both, chyba że podano --dxf-only)");
    println!("  -o <folder_bazowy>  Katalog bazowy (wewnątrz powstaną podfoldery SCX i DXF)");
    println!();
    println!("Opcje technologiczne i DXF:");
    println!("  --material <nazwa>  Wpisuje atrybut Material w <Panel>");
    println!("  --grain <L|W>       Wpisuje kierunek usłojenia (L = oś X, W = oś Y)");
    println!("  --dxf-step-outline  Na warstwie ROZKROJ umieszcza rzeczywisty kontur ze STEPa");
    println!("  --dxf-depth-layers  Tworzy osobne warstwy z głębokością (Wiercenie GW.., Frezowanie GF..)");
    println!("  --dxf-hide-arrow    Ukrywa strzałkę usłojenia w plikach DXF");
    println!("  --include-partial   Uwzględnia częściowe otwory < 360°");
    println!("  --d11-to-d12        Zamienia gzymki Ø11 na Ø12");
    println!("  --no-auto-flip      Wyłącza automatyczne obracanie góra/dół");
    println!("  --keep-empty        Generuje .scx także dla pustych formatek");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        return;
    }

    let cfg_path = PathBuf::from("nanxing_config.ini");
    let cfg = AppConfig::load_or_create(&cfg_path);

    let mut inputs: Vec<PathBuf> = Vec::new();
    let mut base_out_opt: Option<PathBuf> = if !cfg.default_save_dir.trim().is_empty() {
        Some(PathBuf::from(cfg.default_save_dir.trim()))
    } else {
        None
    };
    let mut opts = cfg.to_convert_options();
    let mut dxf_opts = cfg.to_dxf_options();
    let mut mode = CliExportMode::Both;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--out" => {
                i += 1;
                if i < args.len() {
                    base_out_opt = Some(PathBuf::from(&args[i]));
                }
            }
            "--both" | "--dxf" => {
                if mode != CliExportMode::DxfOnly {
                    mode = CliExportMode::Both;
                }
            }
            "--scx-only" => mode = CliExportMode::ScxOnly,
            "--dxf-only" => mode = CliExportMode::DxfOnly,
            "--dxf-step-outline" => dxf_opts.contour_mode = DxfContourMode::StepOutline,
            "--dxf-depth-layers" => dxf_opts.depth_layers = true,
            "--dxf-hide-arrow" => dxf_opts.hide_grain_arrow = true,
            "--material" => {
                i += 1;
                if i < args.len() {
                    opts.material = Some(args[i].clone());
                }
            }
            "--grain" => {
                i += 1;
                if i < args.len() {
                    opts.grain = match args[i].to_ascii_uppercase().as_str() {
                        "L" => GrainDirection::Length,
                        "W" => GrainDirection::Width,
                        _ => GrainDirection::None,
                    };
                }
            }
            "--include-partial" => opts.include_partial_holes = true,
            "--d11-to-d12" => opts.swap_d11_to_d12 = true,
            "--no-auto-flip" => opts.auto_flip_top_bottom = false,
            "--keep-empty" => opts.skip_empty_panels = false,
            other => inputs.push(PathBuf::from(other)),
        }
        i += 1;
    }

    let mut step_files: Vec<PathBuf> = Vec::new();
    for inp in &inputs {
        if inp.is_dir() {
            if let Ok(entries) = fs::read_dir(inp) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if is_step_file(&p) {
                        step_files.push(p);
                    }
                }
            }
        } else if inp.is_file() {
            step_files.push(inp.clone());
        } else {
            eprintln!("BŁĄD: Ścieżka nie istnieje: {}", inp.display());
        }
    }

    if step_files.is_empty() {
        eprintln!("Nie znaleziono żadnych plików .step/.stp do konwersji.");
        std::process::exit(1);
    }

    let do_scx = matches!(mode, CliExportMode::Both | CliExportMode::ScxOnly);
    let do_dxf = matches!(mode, CliExportMode::Both | CliExportMode::DxfOnly);

    let mut scx_saved = 0usize;
    let mut scx_skipped = 0usize;
    let mut dxf_saved = 0usize;
    let mut err_count = 0usize;

    for sf in &step_files {
        let base_dir = base_out_opt
            .clone()
            .unwrap_or_else(|| sf.parent().unwrap_or_else(|| Path::new(".")).to_path_buf());
        let (scx_dir, dxf_dir) = resolve_export_dirs(&base_dir);

        let loaded = match load_step_panel(sf, &opts) {
            Ok(l) => l,
            Err(e) => {
                err_count += 1;
                eprintln!("[BŁĄD] {}: {}", sf.display(), e);
                continue;
            }
        };

        let panel = &loaded.program;
        let mut scx_msg = String::new();
        let mut dxf_msg = String::new();

        if do_scx {
            match save_program_scx_passes(panel, &loaded.file_stem, &scx_dir, &opts) {
                Ok(paths) if !paths.is_empty() => {
                    scx_saved += paths.len();
                    let names: Vec<String> = paths
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect();
                    scx_msg = format!("SCX={}", names.join(" + "));
                }
                Ok(_) => {
                    scx_skipped += 1;
                    scx_msg = "SCX=pominięto(pusta)".to_string();
                }
                Err(e) => {
                    err_count += 1;
                    eprintln!("[BŁĄD SCX] {}: {}", sf.display(), e);
                }
            }
        }

        if do_dxf {
            match save_program_dxf(panel, &loaded.file_stem, &dxf_dir, &dxf_opts) {
                Ok(p) => {
                    dxf_saved += 1;
                    dxf_msg = format!("DXF={}", p.display());
                }
                Err(e) => {
                    err_count += 1;
                    eprintln!("[BŁĄD DXF] {}: {}", sf.display(), e);
                }
            }
        }

        let outputs = [scx_msg.as_str(), dxf_msg.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" | ");

        println!(
            "[OK] {} -> {} | ID={} | {:.1}x{:.1}x{:.1} mm | ops={}",
            sf.file_name().and_then(|s| s.to_str()).unwrap_or(""),
            outputs,
            panel.id,
            panel.length,
            panel.width,
            panel.thickness,
            panel.operations.len(),
        );
    }

    println!(
        "\nPodsumowanie: SCX={} (pominięto={}), DXF={}, błędy={}",
        scx_saved, scx_skipped, dxf_saved, err_count
    );
}

fn is_step_file(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "step" | "stp"))
        .unwrap_or(false)
}
