extern crate clap;
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
            .arg(Arg::with_name("STYLESHEET_FILE").required(true).index(3))
            .arg(Arg::with_name("OSM_IDS").long("osm-id").multiple(true).takes_value(true))
            .get_matches();

    let server_address = matches.value_of("SERVER_ADDRESS").unwrap();
    let geodata_file = matches.value_of("GEODATA_FILE").unwrap();
    let stylesheet_file = matches.value_of("STYLESHEET_FILE").unwrap();
    let osm_ids = matches
        .values_of("OSM_IDS")
        .map(|x| {
            x.map(|y| y.parse().expect(&format!("Invalid OSM ID: {}", y))).collect()
        });

    match run_server(server_address, geodata_file, stylesheet_file, osm_ids) {
        Ok(_) => {},
        Err(e) => {
            for (i, suberror) in e.iter().enumerate() {
                let description = if i == 0 { "Reason" } else { "Caused by" };
                error!("{}: {:?}", description, suberror);
            }
            std::process::exit(1);
        }
    }
}

