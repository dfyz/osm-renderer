#[macro_use] extern crate clap;
extern crate hyper;
#[macro_use] extern crate log;
extern crate slog_stdlog;

extern crate renderer;

use clap::{App, Arg};
use renderer::http_server::run_server;

fn main() {
    slog_stdlog::init().unwrap();

    let matches =
        App::new("OSM renderer server")
            .arg(Arg::with_name("SERVER_ADDRESS").required(true).index(1))
            .arg(Arg::with_name("GEODATA_FILE").required(true).index(2))
            .get_matches();

    let server_address = matches.value_of("SERVER_ADDRESS").unwrap();
    let geodata_file = matches.value_of("GEODATA_FILE").unwrap();

    match run_server(server_address, geodata_file) {
        Ok(_) => {},
        Err(e) => {
            for (i, suberror) in e.iter().enumerate() {
                let description = if i == 0 { "Reason" } else { "Caused by" };
                error!("{}: {}", description, suberror);
            }
            std::process::exit(1);
        }
    }
}

