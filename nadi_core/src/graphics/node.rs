use crate::graphics::color::Color;
use crate::prelude::*;
use abi_stable::std_types::RString;
use abi_stable::StableAbi;
use std::str::FromStr;
use svg::node::element::*;

// TODO make it better later

pub const NODE_COLOR: (&str, Color) = (
    "visual.nodecolor",
    // orange
    Color {
        r: 255,
        g: 165,
        b: 0,
    },
);
pub const LINE_COLOR: (&str, Color) = (
    "visual.linecolor",
    // green
    Color { r: 0, g: 100, b: 0 },
);
pub const TEXT_COLOR: (&str, Color) = (
    "visual.textcolor",
    // blue
    Color {
        r: 102,
        g: 102,
        b: 255,
    },
);
pub const LINE_WIDTH: (&str, f64) = ("visual.linewidth", 1.0);
pub const NODE_SIZE: (&str, f64) = ("visual.nodesize", 5.0);
pub const NODE_SHAPE: (&str, NodeShape) = ("visual.nodeshape", NodeShape::Circle);
pub const DEFAULT_RATIO: f64 = 1.5;

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub enum NodeShape {
    #[default]
    Square,
    Rectangle(f64),
    Circle,
    Triangle,
    IsoTriangle(f64),
    /// Ellipse with ratio of radii
    Ellipse(f64),
    /// Custom path
    SvgPath(RString),
    /// Custom SVG code
    Svg(RString),
    /// Text with rotation
    Text(RString, f64),
}

impl FromStr for NodeShape {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((t, r)) = s.split_once(':') {
            match t {
                "path" => return Ok(Self::SvgPath(r.to_string().into())),
                "svg" => return Ok(Self::Svg(r.to_string().into())),
                _ => (),
            }
            let par: f64 = r
                .parse()
                .map_err(|e| format!("Invalid node shape parameter: {e}"))?;
            match t {
                "rect" | "rectangle" => Ok(Self::Rectangle(par)),
                "triangle" => Ok(Self::IsoTriangle(par)),
                "ellipse" => Ok(Self::Ellipse(par)),
                _ => Ok(Self::Text(t.to_string().into(), par)),
            }
        } else {
            match s {
                "box" | "square" => Ok(Self::Square),
                "rect" | "rectangle" => Ok(Self::Rectangle(DEFAULT_RATIO)),
                "triangle" => Ok(Self::Triangle),
                "circle" => Ok(Self::Circle),
                "ellipse" => Ok(Self::Ellipse(DEFAULT_RATIO)),
                _ => Ok(Self::Text(s.to_string().into(), 0.0)),
            }
        }
    }
}

impl FromAttribute for NodeShape {
    fn from_attr(value: &Attribute) -> Option<Self> {
        FromAttribute::try_from_attr(value).ok()
    }
    fn try_from_attr(value: &Attribute) -> Result<Self, String> {
        Self::from_str(&String::try_from_attr(value)?)
    }
}

impl NodeShape {
    pub fn svg(&self, x: f64, y: f64, size: f64, color: String) -> Element {
        match self {
            NodeShape::Square => Rectangle::new()
                .set("x", x - size / 2.0)
                .set("y", y - size / 2.0)
                .set("height", size)
                .set("width", size)
                .set("fill", color)
                .into(),
            NodeShape::Rectangle(r) => {
                let r = r.abs();
                let (sizex, sizey) = if r > 1.0 {
                    (size / r, size)
                } else {
                    (size, size * r)
                };
                Rectangle::new()
                    .set("x", x - sizex / 2.0)
                    .set("y", y - sizey / 2.0)
                    .set("height", sizey)
                    .set("width", sizex)
                    .set("fill", color)
                    .into()
            }
            NodeShape::Circle => Circle::new()
                .set("cx", x)
                .set("cy", y)
                .set("r", size / 2.0)
                .set("fill", color)
                .into(),
            NodeShape::Ellipse(r) => {
                let r = r.abs();
                let (sizex, sizey) = if r > 1.0 {
                    (size / r, size)
                } else {
                    (size, size * r)
                };
                Ellipse::new()
                    .set("cx", x)
                    .set("cy", y)
                    .set("rx", sizex / 2.0)
                    .set("ry", sizey / 2.0)
                    .set("fill", color)
                    .into()
            }
            NodeShape::Triangle => {
                let ht = 0.8660 * size;
                let dx = size / 2.0;
                let points = [
                    format!("{},{}", x - dx, y + ht / 3.0),
                    format!("{},{}", x, y - 2.0 * ht / 3.0),
                    format!("{},{}", x + dx, y + ht / 3.0),
                ];
                Polygon::new()
                    .set("points", points.join(" "))
                    .set("fill", color)
                    .into()
            }
            NodeShape::IsoTriangle(r) => {
                let ht = 0.8660 * size;
                let dx = size / 2.0;
                let r = r.abs();
                let (ht, dx) = if r > 1.0 { (ht / r, dx) } else { (ht, dx * r) };
                let points = [
                    format!("{},{}", x - dx, y + ht / 3.0),
                    format!("{},{}", x, y - 2.0 * ht / 3.0),
                    format!("{},{}", x + dx, y + ht / 3.0),
                ];
                Polygon::new()
                    .set("points", points.join(" "))
                    .set("fill", color)
                    .into()
            }
            NodeShape::Svg(svg) => SVG::new()
                .set("viewbox", format!("0 0 {size} {size}"))
                .set("fill", color)
                .add(svg::node::Blob::new(svg.to_string()))
                .into(),
            NodeShape::SvgPath(pts) => Path::new()
                .set("d", pts.as_str())
                .set("transform", format!("scale({})", size / 10.0))
                .set("fill", color)
                .into(),
            NodeShape::Text(txt, angle) => Text::new(txt.as_str())
                .set("transform", format!("scale({})", size / 10.0))
                .set("rotate", *angle)
                .set("fill", color)
                .into(),
        }
    }
}

impl NodeInner {
    pub fn node_size(&self) -> f64 {
        self.try_attr_relaxed(NODE_SIZE.0)
            .ok()
            .unwrap_or(NODE_SIZE.1)
    }

    pub fn line_width(&self) -> f64 {
        self.try_attr_relaxed(LINE_WIDTH.0)
            .ok()
            .unwrap_or(LINE_WIDTH.1)
    }

    pub fn set_node_size(&mut self, val: f64) {
        _ = self.set_attr_dot(NODE_SIZE.0, val.into());
    }

    pub fn maybe_node_color(&self) -> Option<Color> {
        self.try_attr::<nadi_core::graphics::color::AttrColor>(NODE_COLOR.0)
            .ok()?
            .color()
            .ok()
    }

    pub fn node_color(&self) -> Color {
        self.maybe_node_color().unwrap_or(NODE_COLOR.1)
    }

    pub fn maybe_text_color(&self) -> Option<Color> {
        self.try_attr::<nadi_core::graphics::color::AttrColor>(TEXT_COLOR.0)
            .ok()?
            .color()
            .ok()
    }

    pub fn text_color(&self) -> Color {
        self.maybe_text_color().unwrap_or(TEXT_COLOR.1)
    }

    pub fn maybe_line_color(&self) -> Option<Color> {
        self.try_attr::<nadi_core::graphics::color::AttrColor>(LINE_COLOR.0)
            .ok()?
            .color()
            .ok()
    }

    pub fn line_color(&self) -> Color {
        self.maybe_line_color().unwrap_or(LINE_COLOR.1)
    }

    pub fn node_shape(&self) -> NodeShape {
        self.try_attr(NODE_SHAPE.0).unwrap_or(NODE_SHAPE.1)
    }

    pub fn node_point(&self, x: f64, y: f64) -> Element {
        self.node_shape()
            .svg(x, y, self.node_size(), self.node_color().hex())
    }

    pub fn node_label(&self, x: f64, y: f64, text: String) -> Text {
        let lab = Text::new(text)
            .set("x", x)
            .set("y", y)
            .set("text-anchor", "start")
            .set("font-size", "large");
        match self.maybe_text_color() {
            Some(c) => lab
                .set("fill", c.hex())
                .set("stroke", c.hex())
                .set("stroke-width", 0.5),
            None => lab,
        }
    }

    pub fn node_line(&self, x1: f64, y1: f64, x2: f64, y2: f64) -> Line {
        Line::new()
            .set("x1", x1)
            .set("y1", y1)
            .set("x2", x2)
            .set("y2", y2)
            .set("stroke-width", self.line_width())
            .set("stroke", self.line_color().hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    #[rstest]
    #[case("box", NodeShape::Square)]
    #[case("square", NodeShape::Square)]
    #[case("circle", NodeShape::Circle)]
    #[case("triangle", NodeShape::Triangle)]
    #[case("rectangle", NodeShape::Rectangle(DEFAULT_RATIO))]
    #[case("ellipse", NodeShape::Ellipse(DEFAULT_RATIO))]
    #[case("rectangle:0.5", NodeShape::Rectangle(0.5))]
    #[case("ellipse:2.0", NodeShape::Ellipse(2.0))]
    fn node_shape_test(#[case] txt: &str, #[case] value: NodeShape) {
        let n = NodeShape::from_str(txt).unwrap();
        assert_eq!(n, value);
    }
}
