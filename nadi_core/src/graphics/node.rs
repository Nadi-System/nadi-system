use crate::graphics::color::Color;
use crate::prelude::*;
use abi_stable::StableAbi;
use std::str::FromStr;
use svg::node::element::*;

// TODO make it better later

pub const NODE_COLOR: (&str, Color) = (
    "visual.nodecolor",
    Color {
        r: 255,
        g: 255,
        b: 255,
    },
);
pub const LINE_COLOR: (&str, Color) = (
    "visual.linecolor",
    Color {
        r: 255,
        g: 150,
        b: 0,
    },
);
pub const TEXT_COLOR: (&str, Color) = (
    "visual.textcolor",
    Color {
        r: 170,
        g: 180,
        b: 200,
    },
);
pub const LINE_WIDTH: (&str, f64) = ("visual.linewidth", 1.0);
pub const NODE_SIZE: (&str, f64) = ("visual.nodesize", 5.0);
pub const NODE_SHAPE: (&str, NodeShape) = ("visual.nodeshape", NodeShape::Square);
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
    Ellipse(f64),
}

impl FromStr for NodeShape {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((t, r)) = s.split_once(':') {
            let size: f64 = r
                .parse()
                .map_err(|e| format!("Invalid Node Size Ratio: {e}"))?;
            match t {
                "rect" | "rectangle" => Ok(Self::Rectangle(size)),
                "triangle" => Ok(Self::IsoTriangle(size)),
                "ellipse" => Ok(Self::Ellipse(size)),
                _ => Err(format!("Unknown shape {t} with size ratio {r}")),
            }
        } else {
            match s {
                "box" | "square" => Ok(Self::Square),
                "rect" | "rectangle" => Ok(Self::Rectangle(DEFAULT_RATIO)),
                "triangle" => Ok(Self::Triangle),
                "circle" => Ok(Self::Circle),
                "ellipse" => Ok(Self::Ellipse(DEFAULT_RATIO)),
                _ => Err(format!("Unknown shape {s}")),
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
    pub fn svg(&self, x: u64, y: u64, size: f64, color: String) -> Element {
        match self {
            NodeShape::Square => Rectangle::new()
                .set("x", x as f64 - size / 2.0)
                .set("y", y as f64 - size / 2.0)
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
                    .set("x", x as f64 - sizex / 2.0)
                    .set("y", y as f64 - sizey / 2.0)
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
                    format!("{},{}", x as f64 - dx, y as f64 + ht / 3.0),
                    format!("{},{}", x as f64, y as f64 - 2.0 * ht / 3.0),
                    format!("{},{}", x as f64 + dx, y as f64 + ht / 3.0),
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
                    format!("{},{}", x as f64 - dx, y as f64 + ht / 3.0),
                    format!("{},{}", x as f64, y as f64 - 2.0 * ht / 3.0),
                    format!("{},{}", x as f64 + dx, y as f64 + ht / 3.0),
                ];
                Polygon::new()
                    .set("points", points.join(" "))
                    .set("fill", color)
                    .into()
            }
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

    pub fn node_color(&self) -> Option<Color> {
        self.try_attr::<nadi_core::graphics::color::AttrColor>(NODE_COLOR.0)
            .ok()
            .unwrap_or_default()
            .color()
            .ok()
    }

    pub fn text_color(&self) -> Option<Color> {
        self.try_attr::<nadi_core::graphics::color::AttrColor>(TEXT_COLOR.0)
            .ok()?
            .color()
            .ok()
    }

    pub fn line_color(&self) -> Option<Color> {
        self.try_attr::<nadi_core::graphics::color::AttrColor>(LINE_COLOR.0)
            .ok()
            .unwrap_or_default()
            .color()
            .ok()
    }

    pub fn node_shape(&self) -> NodeShape {
        self.try_attr(NODE_SHAPE.0).unwrap_or_default()
    }

    pub fn node_point(&self, x: u64, y: u64) -> Element {
        self.node_shape().svg(
            x,
            y,
            self.node_size(),
            self.node_color().unwrap_or_default().hex(),
        )
    }

    pub fn node_label(&self, x: u64, y: u64, text: String) -> Text {
        let lab = Text::new(text)
            .set("x", x)
            .set("y", y)
            .set("text-anchor", "start")
            .set("font-size", "large");
        match self.text_color() {
            Some(c) => lab
                .set("fill", c.hex())
                .set("stroke", c.hex())
                .set("stroke-width", 0.5),
            None => lab,
        }
    }

    pub fn node_line(&self, x1: u64, y1: u64, x2: u64, y2: u64) -> Line {
        Line::new()
            .set("x1", x1)
            .set("y1", y1)
            .set("x2", x2)
            .set("y2", y2)
            .set("stroke-width", self.line_width())
            .set("stroke", self.line_color().unwrap_or_default().hex())
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
