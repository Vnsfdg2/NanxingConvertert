use crate::model::{GrainDirection, Operation, PanelProgram};

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fmt_num(v: f64) -> String {
    let clean = if v.abs() < 1e-6 { 0.0 } else { v };
    let s = format!("{:.4}", clean);
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn render_scx_xml(panel: &PanelProgram) -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n");
    out.push_str("<Root Cad=\"BuiltInCad\" version=\"2.0\">\r\n");
    out.push_str("  <Project>\r\n");
    out.push_str("    <Panels>\r\n");

    let mut extra_attrs = String::new();
    if let Some(ref mat) = panel.material {
        if !mat.trim().is_empty() {
            extra_attrs.push_str(&format!(" Material=\"{}\"", xml_escape(mat)));
        }
    }
    match panel.grain {
        GrainDirection::Length => extra_attrs.push_str(" Grain=\"L\""),
        GrainDirection::Width => extra_attrs.push_str(" Grain=\"W\""),
        GrainDirection::None => {}
    }

    out.push_str(&format!(
        "      <Panel IsProduce=\"true\" ID=\"{}\" Name=\"{}\" Length=\"{}\" Width=\"{}\" Thickness=\"{}\"{} MachiningPoint=\"1\">\r\n",
        xml_escape(&panel.id),
        xml_escape(&panel.name),
        fmt_num(panel.length),
        fmt_num(panel.width),
        fmt_num(panel.thickness),
        extra_attrs
    ));

    // <Outline>
    out.push_str("        <Outline>\r\n");
    for pt in &panel.outline {
        if let Some(r2) = pt.radius2 {
            out.push_str(&format!(
                "          <Point X=\"{}\" Y=\"{}\" Radius2=\"{}\" />\r\n",
                fmt_num(pt.x),
                fmt_num(pt.y),
                fmt_num(r2)
            ));
        } else {
            out.push_str(&format!(
                "          <Point X=\"{}\" Y=\"{}\" />\r\n",
                fmt_num(pt.x),
                fmt_num(pt.y)
            ));
        }
    }
    out.push_str("        </Outline>\r\n");
    out.push_str("        <Innerlines />\r\n");

    // <Machines>
    if panel.operations.is_empty() {
        out.push_str("        <Machines />\r\n");
    } else {
        out.push_str("        <Machines>\r\n");
        for op in &panel.operations {
            match op {
                Operation::HorizontalDrill {
                    face,
                    x,
                    y,
                    z,
                    diameter,
                    depth,
                } => {
                    out.push_str(&format!(
                        "          <Machining Type=\"1\" IsGenCode=\"2\" Face=\"{}\" X=\"{}\" Y=\"{}\" Z=\"{}\" Diameter=\"{}\" Depth=\"{}\" />\r\n",
                        face,
                        fmt_num(*x),
                        fmt_num(*y),
                        fmt_num(*z),
                        fmt_num(*diameter),
                        fmt_num(*depth),
                    ));
                }
                Operation::VerticalDrill {
                    face,
                    x,
                    y,
                    diameter,
                    depth,
                    ..
                } => {
                    out.push_str(&format!(
                        "          <Machining Type=\"2\" IsGenCode=\"2\" Face=\"{}\" X=\"{}\" Y=\"{}\" Diameter=\"{}\" Depth=\"{}\" />\r\n",
                        face,
                        fmt_num(*x),
                        fmt_num(*y),
                        fmt_num(*diameter),
                        fmt_num(*depth),
                    ));
                }
                Operation::LamelloP14 {
                    face,
                    direction,
                    x,
                    y,
                    z,
                    length,
                    width,
                    max_depth,
                } => {
                    out.push_str(&format!(
                        "          <Machining Type=\"1002\" IsGenCode=\"2\" Face=\"{}\" Direction=\"{}\" X=\"{}\" Y=\"{}\" Z=\"{}\" Length=\"{}\" Width=\"{}\" MaxDepth=\"{}\" />\r\n",
                        face,
                        direction,
                        fmt_num(*x),
                        fmt_num(*y),
                        fmt_num(*z),
                        fmt_num(*length),
                        fmt_num(*width),
                        fmt_num(*max_depth),
                    ));
                }
                Operation::Groove {
                    face,
                    x,
                    y,
                    z,
                    end_x,
                    end_y,
                    end_z,
                    depth,
                    width,
                    tool_offset,
                } => {
                    out.push_str(&format!(
                        "          <Machining Type=\"4\" IsGenCode=\"2\" Face=\"{}\" X=\"{}\" Y=\"{}\" Z=\"{}\" EndX=\"{}\" EndY=\"{}\" EndZ=\"{}\" Depth=\"{}\" Width=\"{}\" ToolOffset=\"{}\" />\r\n",
                        face,
                        fmt_num(*x),
                        fmt_num(*y),
                        fmt_num(*z),
                        fmt_num(*end_x),
                        fmt_num(*end_y),
                        fmt_num(*end_z),
                        fmt_num(*depth),
                        fmt_num(*width),
                        xml_escape(tool_offset),
                    ));
                }
            }
        }
        out.push_str("        </Machines>\r\n");
    }

    // <EdgeGroup> (verified mapping: 1=Bottom, 2=Top, 3=Left, 4=Right)
    out.push_str("        <EdgeGroup X1=\"0\" Y1=\"0\">\r\n");
    out.push_str(&format!(
        "          <Edge Face=\"2\" Thickness=\"{}\" Pre_Milling=\"0\" X=\"0\" Y=\"0\" CentralAngle=\"0\" />\r\n",
        fmt_num(panel.edges.face2_top)
    ));
    out.push_str(&format!(
        "          <Edge Face=\"1\" Thickness=\"{}\" Pre_Milling=\"0\" X=\"0\" Y=\"0\" CentralAngle=\"0\" />\r\n",
        fmt_num(panel.edges.face1_bottom)
    ));
    out.push_str(&format!(
        "          <Edge Face=\"4\" Thickness=\"{}\" Pre_Milling=\"0\" X=\"0\" Y=\"0\" CentralAngle=\"0\" />\r\n",
        fmt_num(panel.edges.face4_right)
    ));
    out.push_str(&format!(
        "          <Edge Face=\"3\" Thickness=\"{}\" Pre_Milling=\"0\" X=\"0\" Y=\"0\" CentralAngle=\"0\" />\r\n",
        fmt_num(panel.edges.face3_left)
    ));
    out.push_str("        </EdgeGroup>\r\n");

    out.push_str("      </Panel>\r\n");
    out.push_str("    </Panels>\r\n");
    out.push_str("  </Project>\r\n");
    out.push_str("</Root>\r\n");
    out
}
