use renderer;

use std::env;

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() != 3 {
        let bin_name = args.first().map(String::as_str).unwrap_or("importer");
        eprintln!("Usage: {} INPUT OUTPUT", bin_name);
        std::process::exit(1);
    }

    let input = &args[1];
    let output = &args[2];

    println!("Importing from {} to {}", input, output);

    match renderer::geodata::importer::import(input, output) {
        Ok(_) => println!("All good"),
        Err(err) => {
            for cause in err.iter_chain() {
                eprintln!("{}", cause);
            }
            std::process::exit(1);
        }
    }
}
