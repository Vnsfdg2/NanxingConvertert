use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use nanxing_core::config::AppConfig;
use nanxing_core::convert_step_file;
use nanxing_core::model::GrainDirection;

fn print_usage() {
    println!("NanxingConvertert v0.1 â€” STEP -> SCX (6-stronna wiertarka Nanxing)");
    println!("UĹĽycie:");
    println!("  nanxing_converter <plik.step | folder> [opcje]");
    println!();
    println!("Opcje:");
    println!("  -o <folder>         Folder wyjĹ›ciowy (domyĹ›lnie: <folder_wejĹ›ciowy>\\SCX_NANXING)");
    println!("  --material <nazwa>  Wpisuje atrybut Material w <Panel>");
    println!("  --grain <L|W>       Wpisuje kierunek usĹ‚ojenia Grain");
    println!("  --include-partial   UwzglÄ™dnia czÄ™Ĺ›ciowe otwory < 360Â°");
    println!("  --d11-to-d12        Zamienia gzymki Ă11 na Ă12");
    println!("  --no-auto-flip      WyĹ‚Ä…cza automatyczne obracanie gĂłra/dĂłĹ‚");
    println!("  --keep-empty        Generuje .scx takĹĽe dla pustych formatek");
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
    let mut out_dir_opt: Option<PathBuf> = if !cfg.default_save_dir.trim().is_empty() {
        Some(PathBuf::from(&cfg.default_save_dir))
    } else {
        None
    };
    let mut opts = cfg.to_convert_options();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--out" => {
                i += 1;
                if i < args.len() {
                    out_dir_opt = Some(PathBuf::from(&args[i]));
                }
            }
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
            eprintln!("BĹÄ„D: ĹšcieĹĽka nie istnieje: {}", inp.display());
        }
    }

    if step_files.is_empty() {
        eprintln!("Nie znaleziono ĹĽadnych plikĂłw .step/.stp do konwersji.");
        std::process::exit(1);
    }

    let mut ok_count = 0usize;
    let mut skip_count = 0usize;
    let mut err_count = 0usize;

    for sf in &step_files {
        let out_dir = out_dir_opt.clone().unwrap_or_else(|| {
            sf.parent()
                .unwrap_or_else(|| Path::new("."))
                .join(&cfg.output_subfolder)
        });

        match convert_step_file(sf, &out_dir, &opts) {
            Ok(Some((panel, out_path))) => {
                ok_count += 1;
                println!(
                    "[OK] {} -> {} | ID={} | {:.1}x{:.1}x{:.1} mm | ops={} | flip={}",
                    sf.file_name().and_then(|s| s.to_str()).unwrap_or(""),
                    out_path.display(),
                    panel.id,
                    panel.length,
                    panel.width,
                    panel.thickness,
                    panel.operations.len(),
                    if panel.flipped_top_bottom { "TAK" } else { "NIE" }
                );
                for op in &panel.operations {
                    println!("     - {}", op);
                }
                for w in &panel.warnings {
                    println!("     [!] {}", w);
                }
            }
            Ok(None) => {
                skip_count += 1;
                println!(
                    "[POMINIÄTO] {} (brak operacji obrĂłbczych)",
                    sf.file_name().and_then(|s| s.to_str()).unwrap_or("")
                );
            }
            Err(e) => {
                err_count += 1;
                eprintln!("[BĹÄ„D] {}: {}", sf.display(), e);
            }
        }
    }

    println!(
        "\nPodsumowanie: wygenerowano={}, pominiÄ™to={}, bĹ‚Ä™dy={}",
        ok_count, skip_count, err_count
    );
}

fn is_step_file(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "step" | "stp"))
        .unwrap_or(false)
}

