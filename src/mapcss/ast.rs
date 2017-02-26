#[derive(Debug, Eq, PartialEq)]
pub enum WayType {
    Area,
    Line,
    All,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ObjectType {
    Node,
    Way(WayType),
    Relation,
    Canvas,
    Meta,
    All,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Selector {
    pub object_type: ObjectType,
    pub min_zoom: Option<u8>,
    pub max_zoom: Option<u8>,
}
