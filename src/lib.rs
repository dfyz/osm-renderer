extern crate capnp;
extern crate cairo_sys as cs;
#[macro_use] extern crate error_chain;
extern crate hyper;
extern crate libc;
#[macro_use] extern crate log;
extern crate memmap;
extern crate ordered_float;
extern crate owning_ref;
extern crate png;
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

pub mod mapcss;

pub mod coords;
pub mod draw;
pub mod http_server;
pub mod tile;
