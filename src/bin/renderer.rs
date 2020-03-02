use renderer::http_server::run_server;
use renderer::mapcss::styler::StyleType;
use std::env;
use tini::Ini;

fn fail() -> ! {
    std::process::exit(1);
}

fn get_value_from_config(config: &Ini, section: &str, name: &str) -> String {
    match config.get(section, name) {
        Some(value) => value,
        _ => {
            eprintln!("Property {} is missing in section [{}]", name, section);
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
    let config = match Ini::from_file(config_path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Failed to parse config from {}: {}", config_path, err);
            fail();
        }
    };

    let server_address = get_value_from_config(&config, "http", "address");
    let geodata_file = get_value_from_config(&config, "geodata", "file");

    let style_section = "style";
    let stylesheet_file = get_value_from_config(&config, style_section, "file");
    let stylesheet_type = match get_value_from_config(&config, style_section, "type").as_str() {
        "josm" => StyleType::Josm,
        "mapsme" => StyleType::MapsMe,
        unknown_style => {
            eprintln!("Unknown stylesheet type: {}", unknown_style);
            fail();
        }
    };
    let font_size_multiplier =
        config
            .get::<String>(style_section, "font-mul")
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
        &server_address,
        &geodata_file,
        &stylesheet_file,
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
