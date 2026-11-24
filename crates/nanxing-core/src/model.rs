use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Self {
        let l = self.len();
        if l > 1e-12 {
            Self::new(self.x / l, self.y / l, self.z / l)
        } else {
            Self::new(0.0, 0.0, 0.0)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
    pub radius2: Option<f64>,
}

impl Point2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            radius2: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrainDirection {
    None,
    Length, // L
    Width,  // W
}

#[derive(Debug, Clone, Default)]
pub struct EdgeBanding {
    /// Face="1" -> Dół (Y = 0)
    pub face1_bottom: f64,
    /// Face="2" -> Góra (Y = Width, przy łapach)
    pub face2_top: f64,
    /// Face="3" -> Lewo (X = 0)
    pub face3_left: f64,
    /// Face="4" -> Prawo (X = Length, przy bolcu)
    pub face4_right: f64,
}

impl EdgeBanding {
    pub fn get(&self, face: u8) -> f64 {
        match face {
            1 => self.face1_bottom,
            2 => self.face2_top,
            3 => self.face3_left,
            4 => self.face4_right,
            _ => 0.0,
        }
    }

    pub fn set(&mut self, face: u8, val: f64) {
        match face {
            1 => self.face1_bottom = val,
            2 => self.face2_top = val,
            3 => self.face3_left = val,
            4 => self.face4_right = val,
            _ => {}
        }
    }

    pub fn cycle_face_dir(&mut self, face: u8, forward: bool) {
        const STEPS: [f64; 4] = [0.0, 0.4, 0.8, 2.0];
        let cur = self.get(face);
        let idx = STEPS
            .iter()
            .position(|v| (v - cur).abs() < 0.05)
            .unwrap_or(0);
        let next_idx = if forward {
            (idx + 1) % STEPS.len()
        } else {
            (idx + STEPS.len() - 1) % STEPS.len()
        };
        self.set(face, STEPS[next_idx]);
    }

    pub fn code_label(&self, face: u8) -> &'static str {
        let v = self.get(face);
        if v <= 0.01 {
            "brak"
        } else if (v - 0.4).abs() < 0.1 {
            "PCV 0.4"
        } else if (v - 0.8).abs() < 0.15 {
            "ABS 0.8"
        } else if (v - 2.0).abs() < 0.2 {
            "ABS 2.0"
        } else {
            "OBRZ"
        }
    }

    pub fn active_count(&self) -> usize {
        [
            self.face1_bottom,
            self.face2_top,
            self.face3_left,
            self.face4_right,
        ]
        .into_iter()
        .filter(|&v| v > 0.01)
        .count()
    }
}

#[derive(Debug, Clone)]
pub enum Operation {
    /// Type="1" - horizontal drilling (Face 1..4)
    HorizontalDrill {
        face: u8,
        x: f64,
        y: f64,
        z: f64,
        diameter: f64,
        depth: f64,
    },
    /// Type="2" - vertical drilling (Face 5=top, Face 6=bottom)
    VerticalDrill {
        face: u8,
        x: f64,
        y: f64,
        diameter: f64,
        depth: f64,
        is_through: bool,
    },
    /// Type="1002" - Lamello P14
    LamelloP14 {
        face: u8,
        direction: char, // 'X' or 'Y'
        x: f64,
        y: f64,
        z: f64,
        length: f64,
        width: f64,
        max_depth: f64,
    },
    /// Type="4" - X-axis saw groove
    Groove {
        face: u8,
        x: f64,
        y: f64,
        z: f64,
        end_x: f64,
        end_y: f64,
        end_z: f64,
        depth: f64,
        width: f64,
        tool_offset: String, // "中", "左", "右"
    },
}

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub include_partial_holes: bool,
    pub swap_d11_to_d12: bool,
    pub auto_flip_top_bottom: bool,
    pub skip_empty_panels: bool,
    pub max_side_drill_depth: f64,
    pub horizontal_no_bottom_allowance: f64,
    pub drill_tip_118_max_dia: f64,
    pub lamello_surface_width: f64,
    pub lamello_surface_depth: f64,
    pub lamello_edge_width: f64,
    pub lamello_edge_depth: f64,
    pub lamello_slot_length: f64,
    pub lamello_imbus_diameter: f64,
    pub lamello_imbus_edge_offset: f64,
    pub min_panel_width: f64,
    pub anti_pinch_warning_width: f64,
    pub material: Option<String>,
    pub grain: GrainDirection,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            include_partial_holes: false,
            swap_d11_to_d12: false,
            auto_flip_top_bottom: true,
            skip_empty_panels: true,
            max_side_drill_depth: 50.0,
            horizontal_no_bottom_allowance: 2.0,
            drill_tip_118_max_dia: 12.5,
            lamello_surface_width: 9.8,
            lamello_surface_depth: 14.0,
            lamello_edge_width: 10.15,
            lamello_edge_depth: 14.0,
            lamello_slot_length: 66.0,
            lamello_imbus_diameter: 6.0,
            lamello_imbus_edge_offset: 7.5,
            min_panel_width: 45.0,
            anti_pinch_warning_width: 100.0,
            material: None,
            grain: GrainDirection::None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PanelProgram {
    pub id: String,
    pub name: String,
    pub length: f64,
    pub width: f64,
    pub thickness: f64,
    pub material: Option<String>,
    pub grain: GrainDirection,
    pub outline: Vec<Point2>,
    pub operations: Vec<Operation>,
    pub edges: EdgeBanding,
    pub warnings: Vec<String>,
    pub flipped_top_bottom: bool,
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operation::HorizontalDrill {
                face,
                x,
                y,
                z,
                diameter,
                depth,
            } => write!(
                f,
                "H-Drill F{face} Ø{diameter:.1} d={depth:.2} @ ({x:.2}, {y:.2}, Z={z:.2})"
            ),
            Operation::VerticalDrill {
                face,
                x,
                y,
                diameter,
                depth,
                is_through,
            } => write!(
                f,
                "V-Drill F{face} Ø{diameter:.1} d={depth:.2}{} @ ({x:.2}, {y:.2})",
                if *is_through { " [THRU]" } else { "" }
            ),
            Operation::LamelloP14 {
                face,
                direction,
                x,
                y,
                z,
                width,
                ..
            } => write!(
                f,
                "Lamello P14 F{face} dir={direction} W={width:.2} @ ({x:.2}, {y:.2}, Z={z:.2})"
            ),
            Operation::Groove {
                face,
                x,
                y,
                end_x,
                end_y,
                width,
                depth,
                ..
            } => write!(
                f,
                "Groove F{face} W={width:.1} d={depth:.2} ({x:.1},{y:.1})->({end_x:.1},{end_y:.1})"
            ),
        }
    }
}
