//! Parser plików `.csv` z programów do rozkroju (parowanie obrzeży F1..F4 z formatkami).
//! Zgodny ze zweryfikowanym mapowaniem `<EdgeGroup>` w `NangXinPCDrillCAM`:
//! - Face 1: Dół (Y = 0)
//! - Face 2: Góra (Y = Width)
//! - Face 3: Lewo (X = 0)
//! - Face 4: Prawo (X = Length)

use crate::model::{EdgeBanding, PanelProgram};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn apply_csv_edges_from_file(path: &Path, panels: &mut [PanelProgram]) -> Result<usize, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("Nie można odczytać pliku CSV '{}': {}", path.display(), e))?;
    Ok(apply_csv_edges_from_str(&raw, panels))
}

pub fn apply_csv_edges_from_str(csv_text: &str, panels: &mut [PanelProgram]) -> usize {
    let delimiter = if csv_text.lines().next().unwrap_or("").contains(';') {
        b';'
    } else {
        b','
    };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .has_headers(false)
        .from_reader(csv_text.as_bytes());

    let mut map: HashMap<String, EdgeBanding> = HashMap::new();

    for record in rdr.records().flatten() {
        if record.len() < 5 {
            continue;
        }
        let name = record.get(0).unwrap_or("").trim().to_lowercase();
        if name.is_empty() || matches!(name.as_str(), "id" | "nazwa" | "name" | "formatka") {
            continue;
        }
        let offset = if record.len() >= 8 { 4 } else { 1 };
        let f1 = parse_edge_val(record.get(offset));
        let f2 = parse_edge_val(record.get(offset + 1));
        let f3 = parse_edge_val(record.get(offset + 2));
        let f4 = parse_edge_val(record.get(offset + 3));

        map.insert(
            name,
            EdgeBanding {
                face1_bottom: f1,
                face2_top: f2,
                face3_left: f3,
                face4_right: f4,
            },
        );
    }

    let mut matched = 0;
    for panel in panels.iter_mut() {
        let key_name = panel.name.to_lowercase();
        let key_id = panel.id.to_lowercase();
        if let Some(edges) = map
            .get(&key_name)
            .or_else(|| map.get(&key_id))
            .cloned()
            .or_else(|| {
                map.iter()
                    .find(|(k, _)| {
                        key_name.contains(k.as_str())
                            || k.contains(key_name.as_str())
                            || key_id == *k.as_str()
                    })
                    .map(|(_, v)| v.clone())
            })
        {
            panel.edges = edges;
            matched += 1;
        }
    }
    matched
}

fn parse_edge_val(cell: Option<&str>) -> f64 {
    let raw = cell.unwrap_or("0").trim();
    if raw.is_empty() {
        return 0.0;
    }
    // Obsługa wpisów w stylu "ABS 2.0", "ABS 0.8", "PCV 0.4" lub samej liczby "2.0" / "0,8"
    let cleaned = raw
        .split_whitespace()
        .last()
        .unwrap_or(raw)
        .replace(',', ".");
    cleaned.parse::<f64>().unwrap_or(0.0)
}
