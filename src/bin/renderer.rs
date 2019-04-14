use ini::ini::Properties;
use ini::Ini;
use renderer::http_server::run_server;
use renderer::mapcss::styler::StyleType;
use std::alloc::System;
use std::env;

#[global_allocator]
static GLOBAL: System = System;

fn fail() -> ! {
    std::process::exit(1);
}

type NamedSection<'a, 'b> = (&'a Properties, &'b str);

fn get_section_from_config<'a, 'b>(config: &'a Ini, section_name: &'b str) -> NamedSection<'a, 'b> {
    match config.section(Some(section_name)) {
        Some(section) => (section, section_name),
        _ => {
            eprintln!("The [{}] section is missing", section_name);
            fail();
        }
    }
}

fn get_value_from_config<'a>(section: &'a NamedSection<'_, '_>, name: &str) -> &'a str {
    match section.0.get(name) {
        Some(value) => value,
        _ => {
            eprintln!("Property {} is missing in section [{}]", name, section.1);
            fail();
        }
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() < 2 {
        let bin_name = args.first().map(String::as_str).unwrap_or("renderer");
        eprintln!("Usage: {} CONFIG [OSM_IDS]", bin_name);
        fail();
    }

    let config_path = &args[1];
    let config = match Ini::load_from_file_noescape(config_path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Failed to parse config from {}: {}", config_path, err);
            fail();
        }
    };

    let http_section = get_section_from_config(&config, "http");
    let server_address = get_value_from_config(&http_section, "address");

    let geodata_section = get_section_from_config(&config, "geodata");
    let geodata_file = get_value_from_config(&geodata_section, "file");

    let style_section = get_section_from_config(&config, "style");
    let stylesheet_file = get_value_from_config(&style_section, "file");
    let stylesheet_type = match get_value_from_config(&style_section, "type") {
        "josm" => StyleType::Josm,
        "mapsme" => StyleType::MapsMe,
        unknown_style => {
            eprintln!("Unknown stylesheet type: {}", unknown_style);
            fail();
        }
    };
    let font_size_multiplier = style_section
        .0
        .get("font-mul")
        .map(|multiplier_str| match multiplier_str.parse() {
            Ok(multiplier) => multiplier,
            Err(_) => {
                eprintln!("Invalid font size multiplier: {}", multiplier_str);
                fail();
            }
        });

    let osm_ids = if args.len() >= 3 {
        Some(
            args[2..]
                .iter()
                .map(|x| x.parse().unwrap_or_else(|_| panic!("Invalid OSM ID: {}", x)))
                .collect(),
        )
    } else {
        None
    };

    let res = run_server(
        server_address,
        geodata_file,
        stylesheet_file,
        &stylesheet_type,
        font_size_multiplier,
        osm_ids,
    );

    if let Err(e) = res {
        for cause in e.iter_chain() {
            eprintln!("{}", cause);
        }
        fail();
    }
}
