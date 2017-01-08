extern crate capnp;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate log;

extern crate xml;

pub mod errors {
	error_chain! {}
}

pub mod geodata_capnp {
    include!(concat!(env!("OUT_DIR"), "/geodata_capnp.rs"));
}

pub mod geodata {
    pub mod importer;
}

pub mod coords;
pub mod tile;
