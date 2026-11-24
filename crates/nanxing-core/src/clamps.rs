use crate::config::AppConfig;
use crate::model::{Operation, PanelProgram};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClampBox {
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
}

pub fn compute_clamps(panel: &PanelProgram, cfg: &AppConfig) -> [ClampBox; 2] {
    let a = panel.length.max(1.0);
    let b = panel.width.max(1.0);
    let wc = (a * (cfg.claw_width_pct / 100.0))
        .clamp(cfg.claw_min_mm, cfg.claw_max_mm)
        .min(a * 0.35);
    let k1 = (cfg.claw_pos_pct / 100.0).clamp(0.05, 0.45);
    let k2 = 1.0 - k1;
    [k1, k2].map(|k| ClampBox {
        x0: a * k - wc * 0.5,
        x1: a * k + wc * 0.5,
        y0: b - cfg.claw_grip_mm,
        y1: b + cfg.claw_depth_mm,
    })
}

pub fn op_bbox_mm(op: &Operation, panel: &PanelProgram) -> (f64, f64, f64, f64) {
    match op {
        Operation::VerticalDrill {
            x, y, diameter, ..
        } => {
            let r = diameter * 0.5;
            (*x - r, *y - r, *x + r, *y + r)
        }
        Operation::HorizontalDrill {
            face,
            x,
            y,
            diameter,
            depth,
            ..
        } => {
            let r = diameter * 0.5;
            let (x2, y2) = match face {
                4 => (*x + depth, *y),
                3 => (*x - depth, *y),
                2 => (*x, *y + depth),
                1 => (*x, *y - depth),
                _ => (*x, *y),
            };
            (x.min(x2) - r, y.min(y2) - r, x.max(x2) + r, y.max(y2) + r)
        }
        Operation::LamelloP14 {
            face,
            direction,
            x,
            y,
            length,
            width,
            ..
        } => {
            let (cx, cy) = match face {
                4 => (7.0, *y),
                3 => (panel.length - 7.0, *y),
                2 => (*x, 7.0),
                1 => (*x, panel.width - 7.0),
                _ => (*x, *y),
            };
            let (hw, hh) = if *direction == 'X' {
                (*length * 0.5, *width * 0.5)
            } else {
                (*width * 0.5, *length * 0.5)
            };
            (cx - hw, cy - hh, cx + hw, cy + hh)
        }
        Operation::Groove {
            x,
            y,
            end_x,
            end_y,
            width,
            ..
        } => {
            let h = width * 0.5;
            (
                x.min(*end_x) - h,
                y.min(*end_y) - h,
                x.max(*end_x) + h,
                y.max(*end_y) + h,
            )
        }
    }
}
