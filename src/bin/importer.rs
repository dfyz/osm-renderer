extern crate clap;
#[macro_use]
extern crate log;
extern crate env_logger;

extern crate renderer;

use clap::{App, Arg};

fn main() {
    env_logger::init().unwrap();

    let matches =
        App::new("OSM importer")
            .about("Imports an XML file with OpenStreetMap data to a format suitable for map rendering")
            .arg(Arg::with_name("INPUT")
                    .help("The input XML file")
                    .required(true)
                    .index(1))
            .arg(Arg::with_name("OUTPUT")
                    .help("The output file to convert to")
                    .required(true)
                    .index(2))
            .get_matches();

    let input = matches.value_of("INPUT").unwrap();
    let output = matches.value_of("OUTPUT").unwrap();

    info!("Importing from {} to {}", input, output);

    match renderer::geodata::importer::import(input, output) {
        Ok(_) => info!("All good"),
        Err(err) => {
            error!("Import failed");
            for (i, suberror) in err.iter().enumerate() {
                let description = if i == 0 { "Reason" } else { "Caused by" };
                error!("{}: {}", description, suberror);
            }
            std::process::exit(1);
        }
    }
}
