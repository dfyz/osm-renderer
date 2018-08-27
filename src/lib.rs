extern crate byteorder;
#[macro_use]
extern crate error_chain;
extern crate memmap;
extern crate num_cpus;
extern crate owning_ref;
extern crate png;
extern crate stb_truetype;
extern crate xml;

pub mod errors {
    use std::io;

    error_chain!{
        foreign_links {
            Io(io::Error);
        }
    }
}

pub mod coords;
pub mod draw;
pub mod geodata;
pub mod http_server;
pub mod mapcss;
pub mod tile;
