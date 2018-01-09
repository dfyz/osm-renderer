extern crate cairo_sys as cs;
extern crate capnp;
#[macro_use]
extern crate error_chain;
extern crate futures;
extern crate hyper;
extern crate libc;
#[macro_use]
extern crate log;
extern crate memmap;
extern crate owning_ref;
extern crate png;
extern crate xml;

pub mod errors {
    error_chain!{}
}

pub mod geodata_capnp {
    #![allow(unknown_lints)]
    #![allow(clippy)]
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
