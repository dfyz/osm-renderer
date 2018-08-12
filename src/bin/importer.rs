extern crate error_chain;
extern crate renderer;

use error_chain::ChainedError;
use std::alloc::System;
use std::env;

#[global_allocator]
static GLOBAL: System = System;

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() != 3 {
        let bin_name = args.first().map(|x| x.as_str()).unwrap_or("importer");
        eprintln!("Usage: {} INPUT OUTPUT", bin_name);
        std::process::exit(1);
    }

    let input = &args[1];
    let output = &args[2];

    println!("Importing from {} to {}", input, output);

    match renderer::geodata::importer::import(input, output) {
        Ok(_) => println!("All good"),
        Err(err) => {
            eprintln!("{}", err.display_chain());
            std::process::exit(1);
        }
    }
}
