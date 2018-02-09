extern crate capnp;
#[macro_use]
extern crate error_chain;
extern crate memmap;
extern crate num_cpus;
extern crate owning_ref;
extern crate png;
extern crate xml;

pub mod errors {
    error_chain!{}
}

pub mod geodata_capnp;

pub mod geodata {
    pub mod importer;
    pub mod reader;
}

pub mod mapcss;

pub mod coords;
pub mod draw;
pub mod http_server;
pub mod tile;
