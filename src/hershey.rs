//! Draws SVG text as single pen paths using a Hershey stroke font.
//!
//! The Hershey Fonts were originally created by Dr. A. V. Hershey while working
//! at the U. S. National Bureau of Standards.

/// (left bearing, right bearing, pen paths) for ASCII 32..=126, in Hershey units.
static GLYPHS: [(i8, i8, &[&[(i8, i8)]]); 95] = include!("hershey_glyphs.in");

const BASELINE_UNITS: f32 = 9.0;
const TRACKING_UNITS: f32 = -2.0;
// At font-size 22 this averages 8.6 px per character, the width the prompts budget for.
const SCALE_PER_FONT_PX: f32 = 0.61 / 22.0;
const SHEAR: f32 = 0.18;
const RIGHT_LIMIT: f32 = 752.0;
const DEFAULT_FONT_SIZE: f32 = 22.0;

#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub text: String,
}

/// Fixes the two ways model output has failed to parse: bare ampersands and a missing close.
pub fn repair_svg(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len() + 16);
    for (i, c) in svg.char_indices() {
        if c == '&' {
            let rest = &svg[i + 1..];
            let is_entity = ["amp;", "lt;", "gt;", "quot;", "apos;", "#"].iter().any(|e| rest.starts_with(e));
            out.push_str(if is_entity { "&" } else { "&amp;" });
        } else {
            out.push(c);
        }
    }
    if !out.contains("</svg>") {
        if out.rfind("<text").unwrap_or(0) > out.rfind("</text>").unwrap_or(0) {
            out.push_str("</text>");
        }
        out.push_str("</svg>");
    }
    out
}

fn number(value: &str) -> Option<f32> {
    value.trim().trim_end_matches("px").parse().ok()
}

fn font_size(node: roxmltree::Node) -> f32 {
    node.ancestors()
        .filter_map(|n| n.attribute("font-size"))
        .find_map(number)
        .unwrap_or(DEFAULT_FONT_SIZE)
}

fn node_text(node: roxmltree::Node) -> String {
    node.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect::<String>().trim().to_string()
}

/// Returns None when the SVG has anything but text, so the caller can rasterize it instead.
pub fn parse_text_lines(svg: &str) -> Option<Vec<TextLine>> {
    let doc = roxmltree::Document::parse(svg).ok()?;
    let mut lines = Vec::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        match node.tag_name().name() {
            "svg" | "g" | "tspan" | "title" | "desc" => {}
            "text" => {
                let x = node.attribute("x").and_then(number).unwrap_or(0.0);
                let y = node.attribute("y").and_then(number).unwrap_or(0.0);
                let spans: Vec<_> = node
                    .children()
                    .filter(|n| n.is_element() && n.has_tag_name("tspan") && (n.has_attribute("y") || n.has_attribute("dy")))
                    .collect();
                if spans.is_empty() {
                    lines.push(TextLine { x, y, size: font_size(node), text: node_text(node) });
                    continue;
                }
                let mut line_y = y;
                for span in spans {
                    line_y = match (span.attribute("y").and_then(number), span.attribute("dy").and_then(number)) {
                        (Some(abs), _) => abs,
                        (None, Some(delta)) => line_y + delta,
                        _ => line_y,
                    };
                    lines.push(TextLine {
                        x: span.attribute("x").and_then(number).unwrap_or(x),
                        y: line_y,
                        size: font_size(span),
                        text: node_text(span),
                    });
                }
            }
            _ => return None,
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines)
    }
}

fn glyph(c: char) -> &'static (i8, i8, &'static [&'static [(i8, i8)]]) {
    let c = match c {
        '\u{2018}' | '\u{2019}' => '\'',
        '\u{201C}' | '\u{201D}' => '"',
        '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
        c => c,
    };
    let index = (c as u32).wrapping_sub(32) as usize;
    &GLYPHS[if index < GLYPHS.len() { index } else { '?' as usize - 32 }]
}

fn advance_units(c: char) -> f32 {
    let (left, right, _) = glyph(c);
    (*right as f32 - *left as f32 + TRACKING_UNITS).max(1.0)
}

/// Lays lines out as pen paths in the 768x1024 virtual screen.
pub fn pen_paths(lines: &[TextLine]) -> Vec<Vec<(f32, f32)>> {
    let mut paths = Vec::new();
    for line in lines {
        let scale = line.size * SCALE_PER_FONT_PX;
        let width: f32 = line.text.chars().map(advance_units).sum::<f32>() * scale;
        let squeeze = if line.x + width > RIGHT_LIMIT && width > 0.0 { (RIGHT_LIMIT - line.x) / width } else { 1.0 };
        let mut cursor = line.x;
        for c in line.text.chars() {
            let (left, _, strokes) = glyph(c);
            for stroke in strokes.iter() {
                let path = stroke
                    .iter()
                    .map(|&(px, py)| {
                        let rise = (py as f32 - BASELINE_UNITS) * scale;
                        (cursor + (px as f32 - *left as f32) * scale * squeeze - rise * SHEAR, line.y + rise)
                    })
                    .collect();
                paths.push(path);
            }
            cursor += advance_units(c) * scale * squeeze;
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1024">"#;

    #[test]
    fn repairs_missing_close_and_ampersand() {
        let broken = format!(r#"{SVG}<text x="90" y="100">Q&A and &amp; ok"#);
        let lines = parse_text_lines(&repair_svg(&broken)).unwrap();
        assert_eq!(lines[0].text, "Q&A and & ok");
    }

    #[test]
    fn leaves_valid_svg_alone() {
        let good = format!(r#"{SVG}<text x="90" y="100">fine</text></svg>"#);
        assert_eq!(repair_svg(&good), good);
    }

    #[test]
    fn inherits_size_from_group() {
        let svg = format!(r#"{SVG}<g font-size="30"><text x="90" y="100">a</text></g></svg>"#);
        assert_eq!(parse_text_lines(&svg).unwrap()[0].size, 30.0);
    }

    #[test]
    fn reads_tspans_as_lines() {
        let svg = format!(r#"{SVG}<text x="90" y="100"><tspan x="90" y="100">one</tspan><tspan x="90" dy="29">two</tspan></text></svg>"#);
        let lines = parse_text_lines(&svg).unwrap();
        assert_eq!((lines[1].text.as_str(), lines[1].y), ("two", 129.0));
    }

    #[test]
    fn falls_back_on_shapes() {
        assert!(parse_text_lines(&format!(r#"{SVG}<rect width="5" height="5"/></svg>"#)).is_none());
    }

    #[test]
    fn long_lines_stay_on_screen() {
        let text = "W".repeat(120);
        let paths = pen_paths(&[TextLine { x: 90.0, y: 100.0, size: 22.0, text }]);
        let max_x = paths.iter().flatten().map(|p| p.0).fold(0.0, f32::max);
        assert!(max_x < 768.0, "{max_x}");
    }

    #[test]
    fn budgeted_line_fits_without_squeeze() {
        let text = "the safety decision comes first, then risk: a child can be safe".to_string();
        let paths = pen_paths(&[TextLine { x: 90.0, y: 100.0, size: 22.0, text }]);
        let max_x = paths.iter().flatten().map(|p| p.0).fold(0.0, f32::max);
        assert!(max_x < 710.0, "{max_x}");
    }

    #[test]
    fn unknown_characters_do_not_panic() {
        assert!(!pen_paths(&[TextLine { x: 0.0, y: 50.0, size: 22.0, text: "\u{00e9}\u{2192}".into() }]).is_empty());
    }
}
