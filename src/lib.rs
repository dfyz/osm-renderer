extern crate capnp;
#[macro_use] extern crate error_chain;
extern crate hyper;
#[macro_use] extern crate log;
extern crate memmap;
extern crate owning_ref;
extern crate xml;

pub mod errors {
	error_chain! {}
}

pub mod geodata_capnp {
    include!(concat!(env!("OUT_DIR"), "/geodata_capnp.rs"));
}

pub mod geodata {
    pub mod importer;
    pub mod reader;
}

pub mod coords;
pub mod http_server;
pub mod tile;
