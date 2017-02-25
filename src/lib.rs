extern crate capnp;
extern crate cairo_sys as cs;
#[macro_use] extern crate error_chain;
extern crate hyper;
extern crate lalrpop_util;
extern crate libc;
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

pub mod mapcss;

pub mod coords;
pub mod drawer;
pub mod http_server;
pub mod tile;
