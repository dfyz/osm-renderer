extern crate renderer;

use renderer::http_server::run_server;
use std::env;

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() < 4 {
        let bin_name = args.first().map(|x| x.as_str()).unwrap_or("renderer");
        eprintln!(
            "Usage: {} SERVER_ADDRESS GEODATA_FILE STYLESHEET_FILE [OSM_IDS]",
            bin_name
        );
        std::process::exit(1);
    }

    let server_address = &args[1];
    let geodata_file = &args[2];
    let stylesheet_file = &args[3];
    let osm_ids = if args.len() >= 5 {
        Some(
            args[4..]
                .iter()
                .map(|x| x.parse().expect(&format!("Invalid OSM ID: {}", x)))
                .collect(),
        )
    } else {
        None
    };

    match run_server(server_address, geodata_file, stylesheet_file, osm_ids) {
        Ok(_) => {}
        Err(e) => {
            for (i, suberror) in e.iter().enumerate() {
                let description = if i == 0 { "Reason" } else { "Caused by" };
                eprintln!("{}: {:?}", description, suberror);
            }
            std::process::exit(1);
        }
    }
}
