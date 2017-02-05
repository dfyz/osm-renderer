extern crate capnpc;

use capnpc::CompilerCommand;

fn main() {
	if ::std::env::var("TARGET").unwrap().contains("windows") {
		for extra_lib in &["gdi32", "user32"] {
			println!("cargo:rustc-link-lib=dylib={}", extra_lib);
		}
	}

	CompilerCommand::new()
		.src_prefix("schema")
		.file("schema/geodata.capnp")
		.run()
		.expect("Failed to compile Cap'N'Proto files");
}