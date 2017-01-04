extern crate capnp;

pub mod geodata_capnp {
	include!(concat!(env!("OUT_DIR"), "/geodata_capnp.rs"));
}

pub mod geodata {
	pub mod importer;
}
