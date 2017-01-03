extern crate clap;

use clap::{App,Arg};

fn main() {
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

    println!("Will import from {} to {}", matches.value_of("INPUT").unwrap(), matches.value_of("OUTPUT").unwrap());
}