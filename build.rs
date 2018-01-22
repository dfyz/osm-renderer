extern crate capnpc;

use capnpc::CompilerCommand;

fn main() {
    CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/geodata.capnp")
        .run()
        .expect("Failed to compile Cap'N'Proto files");
}
