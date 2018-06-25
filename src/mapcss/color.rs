#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub fn from_color_name(name: &str) -> Option<Color> {
    match name {
        "white" => Some(Color { r: 255, g: 255, b: 255 }),
        "black" => Some(Color { r: 0, g: 0, b: 0 }),
        "blue" => Some(Color { r: 0, g: 0, b: 255 }),
        "brown" => Some(Color { r: 165, g: 42, b: 42 }),
        "green" => Some(Color { r: 0, g: 255, b: 0 }),
        "grey" => Some(Color { r: 128, g: 128, b: 128 }),
        "pink" => Some(Color { r: 255, g: 192, b: 203 }),
        "purple" => Some(Color { r: 128, g: 0, b: 128 }),
        "red" => Some(Color { r: 255, g: 0, b: 0 }),
        "salmon" => Some(Color { r: 250, g: 128, b: 114 }),
        _ => None,
    }
}
