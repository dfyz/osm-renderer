#[derive(Debug)]
pub enum WayType {
    Area,
    Line,
    All,
}

#[derive(Debug)]
pub enum ObjectType {
    Node,
    Way(WayType),
    Relation,
    Canvas,
    Meta,
    All,
}

pub struct Selector {
    object_type: ObjectType,
}
