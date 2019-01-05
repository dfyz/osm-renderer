#[macro_use]
extern crate error_chain;

use num_cpus;

pub mod errors {
    use std::io;

    error_chain! {
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
