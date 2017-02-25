extern crate capnpc;
extern crate lalrpop;

use capnpc::CompilerCommand;

fn main() {
    let target = ::std::env::var("TARGET").unwrap();
    if target.contains("windows") {
        for extra_lib in &["gdi32", "user32"] {
            println!("cargo:rustc-link-lib=dylib={}", extra_lib);
        }
    }

    if target.contains("darwin") {
        println!("cargo:rustc-link-search=native={}", "/usr/local/lib");
    }

    CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/geodata.capnp")
        .run()
        .expect("Failed to compile Cap'N'Proto files");

    lalrpop::process_root().unwrap();
}