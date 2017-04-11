use mapcss::token::{Color, ZoomLevel};

pub enum ObjectType {
    All,
    Canvas,
    Meta,
    Node,
    Way { should_be_closed: Option<bool> },
}

pub enum UnaryTestType {
    Exists,
    True,
    False,
}

pub enum BinaryTestType {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

pub enum Test {
    Unary { tag_name: String, test_type: UnaryTestType },
    BinaryOp { tag_name: String, value: String, test_type: BinaryTestType },
}

pub enum PropertyValue {
    Identifier(String),
    String(String),
    Numbers(Vec<f64>),
    Color(Color),
}

pub struct Property {
    name: String,
    value: PropertyValue,
}

pub struct Rule {
    object_type: ObjectType,
    min_zoom_range: Option<u8>,
    max_zoom_range: Option<u8>,
    tests: Vec<Test>,
    layer_id: String,
    properties: Vec<Property>,
}