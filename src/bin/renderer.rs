extern crate renderer;

use renderer::geodata::reader::GeodataReader;

fn main() {
    let reader = GeodataReader::new("mow.bin").unwrap();
    println!("{}", reader.get_node_count());
}
