use crate::config::AppConfig;
use crate::lamello_p14::{self, Edge, P14};
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
    // Stała proporcja szerokości do wysokości (96 × 38 mm), odsunięta w całości ponad górną krawędź formatki (Y < 0),
    // aby nie przysłaniać otworów ani oznaczeń obrzeża F2.
    const BASE_W_MM: f64 = 96.0;
    const BASE_H_MM: f64 = 38.0;
    const GAP_ABOVE_MM: f64 = 26.0;
    let uniform = if a < 280.0 {
        (a / 280.0).clamp(0.35, 1.0)
    } else {
        1.0
    };
    let wc = BASE_W_MM * uniform;
    let hc = BASE_H_MM * uniform;
    let gap = GAP_ABOVE_MM * uniform;

    let k1 = (cfg.claw_pos_pct / 100.0).clamp(0.05, 0.45);
    let centers = [a * (1.0 - k1), a * k1];
    centers.map(|cx| ClampBox {
        x0: cx - wc * 0.5,
        x1: cx + wc * 0.5,
        y0: -(gap + hc),
        y1: -gap,
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
            max_depth,
            ..
        } => {
            let mut p14 = P14::default();
            if (1..=4).contains(face) {
                p14.edge_depth = *max_depth;
                let (edge, s0) = match face {
                    1 => (Edge::F3, *x),
                    2 => (Edge::F1, *x),
                    3 => (Edge::F2, *y),
                    _ => (Edge::F4, *y),
                };
                let pts = lamello_p14::edge_slot(&p14, edge, panel.length, panel.width, s0, 20);
                bbox_of_pts(&pts)
            } else {
                let pts = lamello_p14::face_socket(&p14, *x, *y, *direction == 'X');
                bbox_of_pts(&pts)
            }
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

fn bbox_of_pts(pts: &[[f64; 2]]) -> (f64, f64, f64, f64) {
    let mut x0 = f64::INFINITY;
    let mut y0 = f64::INFINITY;
    let mut x1 = f64::NEG_INFINITY;
    let mut y1 = f64::NEG_INFINITY;
    for p in pts {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    (x0, y0, x1, y1)
}
