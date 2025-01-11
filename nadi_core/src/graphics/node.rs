use crate::prelude::*;

use crate::graphics::color::{AttrColor, Color};
use abi_stable::StableAbi;
use cairo::Context;
use std::str::FromStr;

// TODO make it better later

pub const NODE_COLOR: (&str, Color) = (
    "nodecolor",
    Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
    },
);
pub const LINE_COLOR: (&str, Color) = (
    "linecolor",
    Color {
        r: 1.0,
        g: 0.6,
        b: 0.0,
    },
);
pub const TEXT_COLOR: (&str, Color) = (
    "textcolor",
    Color {
        r: 0.7,
        g: 0.8,
        b: 0.9,
    },
);
pub const LINE_WIDTH: (&str, f64) = ("linewidth", 1.0);
pub const NODE_SIZE: (&str, f64) = ("nodesize", 10.0);
pub const NODE_SHAPE: (&str, NodeShape) = ("nodeshape", NodeShape::Square);
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
                "rectangle" => Ok(Self::Rectangle(size)),
                "triangle" => Ok(Self::IsoTriangle(size)),
                "ellipse" => Ok(Self::Ellipse(size)),
                _ => Err(format!("Unknown shape {t} with size ratio {r}")),
            }
        } else {
            match s {
                "box" => Ok(Self::Square),
                "square" => Ok(Self::Square),
                "rectangle" => Ok(Self::Rectangle(DEFAULT_RATIO)),
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

impl NodeInner {
    pub fn draw(&self, ctx: &Context) -> cairo::Result<()> {
        let shape = self
            .attr(NODE_SHAPE.0)
            .and_then(NodeShape::from_attr)
            .unwrap_or_default();
        let size = self
            .attr(NODE_SIZE.0)
            .and_then(f64::from_attr_relaxed)
            .unwrap_or(NODE_SIZE.1);
        match shape {
            NodeShape::Square => {
                let dx = size / 2.0;
                ctx.rel_move_to(-dx, -dx);
                ctx.rel_line_to(0.0, size);
                ctx.rel_line_to(size, 0.0);
                ctx.rel_line_to(0.0, -size);
                ctx.rel_move_to(-size, 0.0);
                ctx.fill()
            }
            NodeShape::Rectangle(r) => {
                let r = r.abs();
                let (sizex, sizey) = if r > 1.0 {
                    (size / r, size)
                } else {
                    (size, size * r)
                };
                ctx.rel_move_to(-sizex / 2.0, -sizey / 2.0);
                ctx.rel_line_to(0.0, sizey);
                ctx.rel_line_to(sizex, 0.0);
                ctx.rel_line_to(0.0, -sizey);
                ctx.rel_move_to(-sizex, 0.0);
                ctx.fill()
            }
            NodeShape::Circle => {
                let (xc, yc) = ctx.current_point()?;
                ctx.arc(xc, yc, size / 2.0, 0.0, 2.0 * 3.1416);
                ctx.fill()
            }
            NodeShape::Ellipse(r) => {
                let m = ctx.matrix();
                let (xc, yc) = ctx.current_point()?;
                ctx.translate(xc, yc);
                let r = r.abs();
                if r > 1.0 {
                    ctx.scale(1.0 / r, 1.0);
                } else {
                    ctx.scale(1.0, r);
                };
                ctx.arc(0.0, 0.0, size / 2.0, 0.0, 2.0 * 3.1416);
                ctx.set_matrix(m);
                // revert the scale
                // if r > 1.0 {
                //     ctx.scale(r, 1.0);
                // } else {
                //     ctx.scale(1.0, 1.0 / r);
                // };
                ctx.fill()
            }
            NodeShape::Triangle => {
                let ht = 0.8660 * size;
                let dx = size / 2.0;
                ctx.rel_move_to(-dx, ht / 3.0);
                ctx.rel_line_to(dx, -ht);
                ctx.rel_line_to(dx, ht);
                ctx.fill()
            }
            NodeShape::IsoTriangle(r) => {
                let ht = 0.8660 * size;
                let dx = size / 2.0;
                let r = r.abs();
                let (ht, dx) = if r > 1.0 { (ht / r, dx) } else { (ht, dx * r) };
                ctx.rel_move_to(-dx, ht / 3.0);
                ctx.rel_line_to(dx, ht);
                ctx.rel_line_to(dx, -ht);
                ctx.fill()
            }
        }
    }

    pub fn draw_color(&self, ctx: &Context) -> cairo::Result<()> {
        self.set_color_attr(ctx, NODE_COLOR.0, &NODE_COLOR.1);
        self.draw(ctx)
    }

    pub fn set_color_attr(&self, ctx: &Context, attr: &str, default: &Color) {
        let c = self.try_attr::<AttrColor>(attr).unwrap_or_default();
        match c.color() {
            Ok(c) => c.set(ctx),
            Err(e) => {
                eprintln!("{e}");
                default.set(ctx)
            }
        }
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
