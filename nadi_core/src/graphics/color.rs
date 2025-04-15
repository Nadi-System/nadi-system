use cairo::Context;
use nadi_core::nadi_plugin::FromAttribute;
use nadi_core::prelude::*;

/// color can be 0.0-1.0; 0-255, [r, g, b], {r=.,g=.,b=.} or name
#[derive(Debug, Clone, FromAttribute)]
pub enum AttrColor {
    MonoInt(i64),
    Mono(f64),
    Named(String),
    Rgb((f64, f64, f64)),
    RgbNamed(Color),
}

impl std::default::Default for AttrColor {
    fn default() -> Self {
        Self::RgbNamed(Color::default())
    }
}

impl AttrColor {
    pub fn color(self) -> Result<Color, String> {
        let (r, g, b) = match self {
            Self::MonoInt(i) => {
                let v = i as f64 / 255.0;
                (v, v, v)
            }
            Self::Mono(v) => (v, v, v),
            Self::RgbNamed(c) => return Ok(c),
            Self::Named(n) => color_by_name(&n).ok_or(format!("Invalid Color name {n:?}"))?,
            Self::Rgb(v) => v,
        };
        Ok(Color { r, g, b })
    }
}

#[derive(Default, Debug, Clone, FromAttribute)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Color {
    pub fn set(&self, ctx: &Context) {
        ctx.set_source_rgb(self.r, self.g, self.b);
    }
}

// copied from named_colors crate
fn color_by_name(name: &str) -> Option<(f64, f64, f64)> {
    let (r, g, b) = match name {
        "red" => (255, 0, 0),
        "green" => (0, 255, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "cyan" => (0, 255, 255),
        "magenta" => (255, 0, 255),
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "orange" => (255, 165, 0),
        "pink" => (255, 192, 203),
        "purple" => (128, 0, 128),
        "brown" => (165, 42, 42),
        "gray" => (128, 128, 128),
        "navy" => (0, 0, 128),
        "teal" => (0, 128, 128),
        "lime" => (0, 255, 0),
        "olive" => (128, 128, 0),
        "maroon" => (128, 0, 0),
        "silver" => (192, 192, 192),
        "lightgray" => (238, 238, 238),
        "lightblue" => (173, 216, 230),
        "lightgreen" => (144, 238, 144),
        "lightyellow" => (255, 255, 224),
        "lightcyan" => (224, 255, 255),
        "lightmagenta" => (255, 205, 205),
        "lightpurple" => (219, 112, 147),
        "lightbrown" => (255, 16, 255),
        "lightnavy" => (255, 0, 255),
        "lightteal" => (178, 178, 178),
        "lightlime" => (255, 255, 0),
        "lightolive" => (152, 251, 152),
        "lightmaroon" => (199, 21, 133),
        "lightsilver" => (205, 205, 205),
        "lightlightgray" => (245, 245, 245),
        "lightlightblue" => (221, 238, 245),
        "lightlightgreen" => (218, 228, 181),
        "lightlightyellow" => (255, 255, 224),
        "lightlightcyan" => (240, 255, 255),
        "lightlightmagenta" => (255, 229, 229),
        "lightlightpurple" => (238, 154, 191),
        "lightlightbrown" => (255, 170, 204),
        "khaki" => (240, 230, 140),
        "lavender" => (230, 230, 250),
        "coral" => (255, 127, 80),
        "salmon" => (250, 128, 114),
        "turquoise" => (64, 224, 208),
        "plum" => (221, 160, 221),
        "gold" => (255, 215, 0),
        "chocolate" => (210, 105, 30),
        "firebrick" => (178, 34, 34),
        "indigo" => (75, 0, 130),
        "ivory" => (255, 255, 240),
        "limegreen" => (50, 205, 50),
        "orchid" => (218, 112, 214),
        "peru" => (205, 133, 63),
        "powderblue" => (176, 224, 230),
        "rosybrown" => (188, 143, 143),
        "seagreen" => (46, 139, 87),
        "sienna" => (160, 82, 45),
        "tan" => (210, 180, 140),
        "crimson" => (220, 20, 60),
        "darkblue" => (0, 0, 139),
        "darkgreen" => (0, 100, 0),
        "darkkhaki" => (189, 183, 107),
        "darkmagenta" => (139, 0, 139),
        "darkorchid" => (153, 50, 204),
        "darkred" => (139, 0, 0),
        "darkslateblue" => (72, 61, 139),
        "darkslategray" => (47, 79, 79),
        "deeppink" => (255, 20, 147),
        "deepskyblue" => (0, 191, 255),
        "dimgray" => (105, 105, 105),
        "dodgerblue" => (30, 144, 255),
        "gainsboro" => (220, 220, 220),
        "ghostwhite" => (248, 248, 255),
        "honeydew" => (240, 255, 240),
        "lightcoral" => (240, 128, 128),
        "mistyrose" => (255, 228, 225),
        "palegoldenrod" => (238, 232, 170),
        "paleturquoise" => (175, 238, 238),
        "palevioletred" => (219, 112, 147),
        "papayawhip" => (255, 239, 213),
        "peachpuff" => (255, 218, 185),
        "rebeccapurple" => (102, 51, 153),
        "slateblue" => (106, 90, 205),
        "slategray" => (112, 128, 144),
        "snow" => (255, 250, 250),
        "thistle" => (216, 191, 216),
        "tomato" => (255, 99, 71),
        "wheat" => (245, 222, 179),
        "mediumseagreen" => (60, 179, 113),
        "mediumslateblue" => (123, 104, 238),
        "mediumspringgreen" => (0, 250, 154),
        "mediumturquoise" => (72, 209, 204),
        "mediumvioletred" => (199, 21, 133),
        "midnightblue" => (25, 25, 112),
        "mintcream" => (245, 255, 250),
        "moccasin" => (255, 228, 181),
        "navajowhite" => (255, 222, 173),
        "oldlace" => (253, 245, 230),
        "olivedrab" => (107, 142, 35),
        "orangered" => (255, 69, 0),
        "palegreen" => (152, 251, 152),
        "royalblue" => (65, 105, 225),
        "saddlebrown" => (139, 69, 19),
        "sandybrown" => (244, 164, 96),
        "seashell" => (255, 245, 238),
        "skyblue" => (135, 206, 235),
        "springgreen" => (0, 255, 127),
        "steelblue" => (70, 130, 180),
        "violet" => (238, 130, 238),
        "whitesmoke" => (245, 245, 245),
        "yellowgreen" => (154, 205, 50),
        "bisque" => (255, 228, 196),
        "blanchedalmond" => (255, 235, 205),
        "burlywood" => (222, 184, 135),
        "cadetblue" => (95, 158, 160),
        "cornflowerblue" => (100, 149, 237),
        "cornsilk" => (255, 248, 220),
        "darkcyan" => (0, 139, 139),
        "darkgoldenrod" => (184, 134, 11),
        "darkgray" => (169, 169, 169),
        "darkorange" => (255, 140, 0),
        "darksalmon" => (233, 150, 122),
        "darkseagreen" => (143, 188, 143),
        "darkturquoise" => (0, 206, 209),
        "darkviolet" => (148, 0, 211),
        "floralwhite" => (255, 250, 240),
        "forestgreen" => (34, 139, 34),
        "goldenrod" => (218, 165, 32),
        "greenyellow" => (173, 255, 47),
        "hotpink" => (255, 105, 180),
        "indianred" => (205, 92, 92),
        _ => return None,
    };

    let r = r as f64 / 255.0;
    let g = g as f64 / 255.0;
    let b = b as f64 / 255.0;
    // Some(Color{r, g, b})
    Some((r, g, b))
}
