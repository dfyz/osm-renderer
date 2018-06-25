extern crate error_chain;
extern crate renderer;

use error_chain::ChainedError;
use renderer::http_server::run_server;
use renderer::mapcss::styler::StyleType;
use std::env;

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() < 5 {
        let bin_name = args.first().map(|x| x.as_str()).unwrap_or("renderer");
        eprintln!(
            "Usage: {} SERVER_ADDRESS GEODATA_FILE STYLESHEET_FILE STYLESHEET_TYPE [OSM_IDS]",
            bin_name
        );
        std::process::exit(1);
    }

    let server_address = &args[1];
    let geodata_file = &args[2];
    let stylesheet_file = &args[3];
    let stylesheet_type = match args[4].as_str() {
        "josm" => StyleType::Josm,
        "mapsme" => StyleType::MapsMe,
        unknown_style => {
            eprintln!("Unknown stylesheet type: {}", unknown_style);
            std::process::exit(1);
        }
    };
    let osm_ids = if args.len() >= 6 {
        Some(
            args[5..]
                .iter()
                .map(|x| x.parse().unwrap_or_else(|_| panic!("Invalid OSM ID: {}", x)))
                .collect(),
        )
    } else {
        None
    };

    let res = run_server(server_address, geodata_file, stylesheet_file, &stylesheet_type, osm_ids);

    if let Err(e) = res {
        eprintln!("{}", e.display_chain());
        std::process::exit(1);
    }
}
