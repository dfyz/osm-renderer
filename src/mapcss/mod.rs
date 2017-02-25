mod grammar;
mod selector;

#[test]
fn test() {
    println!("{:?}", grammar::parse_ObjectType("node"));
    println!("{:?}", grammar::parse_ObjectType("way"));
    println!("{:?}", grammar::parse_ObjectType("object"));
}
