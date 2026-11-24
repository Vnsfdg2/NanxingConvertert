use std::fs;
use std::path::Path;

use crate::model::{ConvertOptions, GrainDirection};

#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    // [Paths]
    pub default_open_dir: String,
    pub default_save_dir: String,
    pub default_csv_dir: String,
    pub output_subfolder: String,

    // [LamelloP14]
    pub lamello_surface_width: f64,
    pub lamello_surface_depth: f64,
    pub lamello_edge_width: f64,
    pub lamello_edge_depth: f64,
    pub lamello_slot_length: f64,
    pub lamello_imbus_diameter: f64,
    pub lamello_imbus_edge_offset: f64,

    // [Drilling]
    pub include_partial_holes: bool,
    pub swap_d11_to_d12: bool,
    pub auto_flip_top_bottom: bool,
    pub skip_empty_panels: bool,
    pub compensate_drill_cone: bool,
    pub limit_side_drill: bool,
    pub max_side_drill_depth: f64,
    pub horizontal_no_bottom_allowance: f64,
    pub drill_tip_118_max_dia: f64,

    // [Machine]
    pub min_panel_width: f64,
    pub anti_pinch_warning_width: f64,
    pub saw_blade_thickness: f64,
    pub default_groove_width: f64,
    pub bottom_spindle_dia: f64,

    // [Clamping] (parametry łap i bolca z makiety rev2)
    pub claw_pos_pct: f64,
    pub claw_width_pct: f64,
    pub claw_min_mm: f64,
    pub claw_max_mm: f64,
    pub claw_grip_mm: f64,
    pub claw_depth_mm: f64,
    pub pin_dx_mm: f64,
    pub pin_dy_mm: f64,
    pub pin_dia_mm: f64,

    // [Viewport]
    pub grid_step_mm: f64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_open_dir: r"D:\WorkerResource\ALKATRAS_BSOLID".to_string(),
            default_save_dir: String::new(),
            default_csv_dir: r"D:\WorkerResource\ALKATRAS_BSOLID".to_string(),
            output_subfolder: "SCX_NANXING".to_string(),

            lamello_surface_width: 9.8,
            lamello_surface_depth: 14.0,
            lamello_edge_width: 10.15,
            lamello_edge_depth: 14.0,
            lamello_slot_length: 66.0,
            lamello_imbus_diameter: 6.0,
            lamello_imbus_edge_offset: 7.5,

            include_partial_holes: false,
            swap_d11_to_d12: false,
            auto_flip_top_bottom: true,
            skip_empty_panels: true,
            compensate_drill_cone: true,
            limit_side_drill: true,
            max_side_drill_depth: 50.0,
            horizontal_no_bottom_allowance: 2.0,
            drill_tip_118_max_dia: 12.5,

            min_panel_width: 45.0,
            anti_pinch_warning_width: 100.0,
            saw_blade_thickness: 4.0,
            default_groove_width: 6.0,
            bottom_spindle_dia: 12.0,

            claw_pos_pct: 20.0,
            claw_width_pct: 16.0,
            claw_min_mm: 36.0,
            claw_max_mm: 130.0,
            claw_grip_mm: 8.0,
            claw_depth_mm: 36.0,
            pin_dx_mm: 20.0,
            pin_dy_mm: 32.0,
            pin_dia_mm: 14.0,

            grid_step_mm: 100.0,
        }
    }
}

impl AppConfig {
    pub fn load_or_create(path: &Path) -> Self {
        if let Ok(text) = fs::read_to_string(path) {
            Self::parse_ini(&text)
        } else {
            let cfg = Self::default();
            let _ = cfg.save(path);
            cfg
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let content = self.to_ini();
        fs::write(path, content.as_bytes())
            .map_err(|e| format!("Błąd zapisu konfiguracji '{}': {}", path.display(), e))
    }

    pub fn to_convert_options(&self) -> ConvertOptions {
        ConvertOptions {
            include_partial_holes: self.include_partial_holes,
            swap_d11_to_d12: self.swap_d11_to_d12,
            auto_flip_top_bottom: self.auto_flip_top_bottom,
            skip_empty_panels: self.skip_empty_panels,
            max_side_drill_depth: if self.limit_side_drill {
                self.max_side_drill_depth
            } else {
                9999.0
            },
            horizontal_no_bottom_allowance: self.horizontal_no_bottom_allowance,
            drill_tip_118_max_dia: if self.compensate_drill_cone {
                self.drill_tip_118_max_dia
            } else {
                0.0
            },
            lamello_surface_width: self.lamello_surface_width,
            lamello_surface_depth: self.lamello_surface_depth,
            lamello_edge_width: self.lamello_edge_width,
            lamello_edge_depth: self.lamello_edge_depth,
            lamello_slot_length: self.lamello_slot_length,
            lamello_imbus_diameter: self.lamello_imbus_diameter,
            lamello_imbus_edge_offset: self.lamello_imbus_edge_offset,
            min_panel_width: self.min_panel_width,
            anti_pinch_warning_width: self.anti_pinch_warning_width,
            material: None,
            grain: GrainDirection::None,
        }
    }

    pub fn to_ini(&self) -> String {
        format!(
            "# NanxingConvertert — plik konfiguracyjny\r\n\
             [Paths]\r\n\
             default_open_dir = {}\r\n\
             default_save_dir = {}\r\n\
             default_csv_dir = {}\r\n\
             output_subfolder = {}\r\n\
             \r\n\
             [LamelloP14]\r\n\
             surface_width = {}\r\n\
             surface_depth = {}\r\n\
             edge_width = {}\r\n\
             edge_depth = {}\r\n\
             slot_length = {}\r\n\
             imbus_diameter = {}\r\n\
             imbus_edge_offset = {}\r\n\
             \r\n\
             [Drilling]\r\n\
             include_partial_holes = {}\r\n\
             swap_d11_to_d12 = {}\r\n\
             auto_flip_top_bottom = {}\r\n\
             skip_empty_panels = {}\r\n\
             compensate_drill_cone = {}\r\n\
             limit_side_drill = {}\r\n\
             max_side_drill_depth = {}\r\n\
             horizontal_no_bottom_allowance = {}\r\n\
             drill_tip_118_max_dia = {}\r\n\
             \r\n\
             [Machine]\r\n\
             min_panel_width = {}\r\n\
             anti_pinch_warning_width = {}\r\n\
             saw_blade_thickness = {}\r\n\
             default_groove_width = {}\r\n\
             bottom_spindle_dia = {}\r\n\
             \r\n\
             [Clamping]\r\n\
             claw_pos_pct = {}\r\n\
             claw_width_pct = {}\r\n\
             claw_min_mm = {}\r\n\
             claw_max_mm = {}\r\n\
             claw_grip_mm = {}\r\n\
             claw_depth_mm = {}\r\n\
             pin_dx_mm = {}\r\n\
             pin_dy_mm = {}\r\n\
             pin_dia_mm = {}\r\n\
             \r\n\
             [Viewport]\r\n\
             grid_step_mm = {}\r\n",
            self.default_open_dir,
            self.default_save_dir,
            self.default_csv_dir,
            self.output_subfolder,
            self.lamello_surface_width,
            self.lamello_surface_depth,
            self.lamello_edge_width,
            self.lamello_edge_depth,
            self.lamello_slot_length,
            self.lamello_imbus_diameter,
            self.lamello_imbus_edge_offset,
            self.include_partial_holes,
            self.swap_d11_to_d12,
            self.auto_flip_top_bottom,
            self.skip_empty_panels,
            self.compensate_drill_cone,
            self.limit_side_drill,
            self.max_side_drill_depth,
            self.horizontal_no_bottom_allowance,
            self.drill_tip_118_max_dia,
            self.min_panel_width,
            self.anti_pinch_warning_width,
            self.saw_blade_thickness,
            self.default_groove_width,
            self.bottom_spindle_dia,
            self.claw_pos_pct,
            self.claw_width_pct,
            self.claw_min_mm,
            self.claw_max_mm,
            self.claw_grip_mm,
            self.claw_depth_mm,
            self.pin_dx_mm,
            self.pin_dy_mm,
            self.pin_dia_mm,
            self.grid_step_mm,
        )
    }

    pub fn from_ini(text: &str) -> Self {
        Self::parse_ini(text)
    }

    fn parse_ini(text: &str) -> Self {
        let mut cfg = Self::default();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty()
                || line.starts_with('#')
                || line.starts_with(';')
                || line.starts_with('[')
            {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let key = k.trim();
            let val = v.trim();
            match key {
                "default_open_dir" => cfg.default_open_dir = val.to_string(),
                "default_save_dir" => cfg.default_save_dir = val.to_string(),
                "default_csv_dir" => cfg.default_csv_dir = val.to_string(),
                "output_subfolder" => cfg.output_subfolder = val.to_string(),
                "surface_width" => {
                    if let Ok(n) = val.parse() {
                        cfg.lamello_surface_width = n;
                    }
                }
                "surface_depth" => {
                    if let Ok(n) = val.parse() {
                        cfg.lamello_surface_depth = n;
                    }
                }
                "edge_width" => {
                    if let Ok(n) = val.parse() {
                        cfg.lamello_edge_width = n;
                    }
                }
                "edge_depth" => {
                    if let Ok(n) = val.parse() {
                        cfg.lamello_edge_depth = n;
                    }
                }
                "slot_length" => {
                    if let Ok(n) = val.parse() {
                        cfg.lamello_slot_length = n;
                    }
                }
                "imbus_diameter" => {
                    if let Ok(n) = val.parse() {
                        cfg.lamello_imbus_diameter = n;
                    }
                }
                "imbus_edge_offset" => {
                    if let Ok(n) = val.parse() {
                        cfg.lamello_imbus_edge_offset = n;
                    }
                }
                "include_partial_holes" => {
                    if let Ok(b) = val.parse() {
                        cfg.include_partial_holes = b;
                    }
                }
                "swap_d11_to_d12" => {
                    if let Ok(b) = val.parse() {
                        cfg.swap_d11_to_d12 = b;
                    }
                }
                "auto_flip_top_bottom" => {
                    if let Ok(b) = val.parse() {
                        cfg.auto_flip_top_bottom = b;
                    }
                }
                "skip_empty_panels" => {
                    if let Ok(b) = val.parse() {
                        cfg.skip_empty_panels = b;
                    }
                }
                "compensate_drill_cone" => {
                    if let Ok(b) = val.parse() {
                        cfg.compensate_drill_cone = b;
                    }
                }
                "limit_side_drill" => {
                    if let Ok(b) = val.parse() {
                        cfg.limit_side_drill = b;
                    }
                }
                "max_side_drill_depth" => {
                    if let Ok(n) = val.parse() {
                        cfg.max_side_drill_depth = n;
                    }
                }
                "horizontal_no_bottom_allowance" => {
                    if let Ok(n) = val.parse() {
                        cfg.horizontal_no_bottom_allowance = n;
                    }
                }
                "drill_tip_118_max_dia" => {
                    if let Ok(n) = val.parse() {
                        cfg.drill_tip_118_max_dia = n;
                    }
                }
                "min_panel_width" => {
                    if let Ok(n) = val.parse() {
                        cfg.min_panel_width = n;
                    }
                }
                "anti_pinch_warning_width" => {
                    if let Ok(n) = val.parse() {
                        cfg.anti_pinch_warning_width = n;
                    }
                }
                "saw_blade_thickness" => {
                    if let Ok(n) = val.parse() {
                        cfg.saw_blade_thickness = n;
                    }
                }
                "default_groove_width" => {
                    if let Ok(n) = val.parse() {
                        cfg.default_groove_width = n;
                    }
                }
                "bottom_spindle_dia" => {
                    if let Ok(n) = val.parse() {
                        cfg.bottom_spindle_dia = n;
                    }
                }
                "claw_pos_pct" => {
                    if let Ok(n) = val.parse() {
                        cfg.claw_pos_pct = n;
                    }
                }
                "claw_width_pct" => {
                    if let Ok(n) = val.parse() {
                        cfg.claw_width_pct = n;
                    }
                }
                "claw_min_mm" => {
                    if let Ok(n) = val.parse() {
                        cfg.claw_min_mm = n;
                    }
                }
                "claw_max_mm" => {
                    if let Ok(n) = val.parse() {
                        cfg.claw_max_mm = n;
                    }
                }
                "claw_grip_mm" => {
                    if let Ok(n) = val.parse() {
                        cfg.claw_grip_mm = n;
                    }
                }
                "claw_depth_mm" => {
                    if let Ok(n) = val.parse() {
                        cfg.claw_depth_mm = n;
                    }
                }
                "pin_dx_mm" => {
                    if let Ok(n) = val.parse() {
                        cfg.pin_dx_mm = n;
                    }
                }
                "pin_dy_mm" => {
                    if let Ok(n) = val.parse() {
                        cfg.pin_dy_mm = n;
                    }
                }
                "pin_dia_mm" => {
                    if let Ok(n) = val.parse() {
                        cfg.pin_dia_mm = n;
                    }
                }
                "grid_step_mm" => {
                    if let Ok(n) = val.parse() {
                        cfg.grid_step_mm = n;
                    }
                }
                _ => {}
            }
        }
        cfg
    }
}
