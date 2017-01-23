#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate slog_stdlog;

extern crate renderer;

use clap::{App, Arg};
use renderer::geodata::reader::OsmEntity;

fn get_command_line_arg_value<T: std::str::FromStr>(matches: &clap::ArgMatches, arg: &str) -> T {
    match value_t!(matches.value_of(arg), T) {
        Ok(val) => val,
        Err(e) => {
            error!("Failed to parse {}: {}", arg, e);
            std::process::exit(1);
        },
    }
}

fn get_name<'a>(entity: &OsmEntity<'a>) -> Option<&'a str> {
    let tags = entity.tags();
    tags.get_by_key("name")
}

fn print_tile_contents(geodata_file: &str, tile: renderer::tile::Tile) -> renderer::errors::Result<()> {
    let reader = renderer::geodata::reader::GeodataReader::new(geodata_file)?;

    let entities = reader.get_entities_in_tile(&tile);

    let mut unnamed_node_count = 0;
    let mut unnamed_way_count = 0;
    let mut unnamed_relation_count = 0;

    for node in entities.nodes {
        match get_name(&node) {
            Some(value) => {
                info!("NODE: {}", value);
            },
            None => unnamed_node_count += 1,
        };
    }

    for way in entities.ways {
        match get_name(&way) {
            Some(value) => {
                info!("WAY: {} ({} nodes)", value, way.node_count());
            },
            None => unnamed_way_count += 1,
        }
    }

    for relation in entities.relations {
        match get_name(&relation) {
            Some(value) => {
                info!("RELATION: {} ({} ways, {} nodes)", value, relation.way_count(), relation.node_count());
            },
            None => unnamed_relation_count += 1,
        }
    }

    info!("Unnamed: {} nodes, {} ways, {} relations", unnamed_node_count, unnamed_way_count, unnamed_relation_count);

    Ok(())
}

fn main() {
    slog_stdlog::init().unwrap();

    let matches =
        App::new("OSM renderer")
            .arg(Arg::with_name("ZOOM").required(true).index(1))
            .arg(Arg::with_name("X").required(true).index(2))
            .arg(Arg::with_name("Y").required(true).index(3))
            .arg(Arg::with_name("GEODATA_FILE").required(true).index(4))
            .get_matches();

    let zoom = get_command_line_arg_value(&matches, "ZOOM");
    let x = get_command_line_arg_value(&matches, "X");
    let y = get_command_line_arg_value(&matches, "Y");
    let geodata_file = matches.value_of("GEODATA_FILE").unwrap();

    let tile = renderer::tile::Tile {
        zoom: zoom,
        x: x,
        y: y,
    };

    match print_tile_contents(geodata_file, tile) {
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

