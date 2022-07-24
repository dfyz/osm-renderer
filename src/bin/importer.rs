use anyhow::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn import(input: &Path, tmp_output: &Path, output: &Path) -> Result<()> {
    println!("Importing OSM data from {}", input.to_string_lossy());
    renderer::geodata::importer::import(input, tmp_output)?;
    fs::rename(tmp_output, output)?;

    Ok(())
}

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() != 3 {
        let bin_name = args.first().map(String::as_str).unwrap_or("importer");
        eprintln!("Usage: {} INPUT OUTPUT", bin_name);
        std::process::exit(1);
    }

    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);

    let mut tmp_output = output.clone();
    tmp_output.set_extension("tmp");

    match import(&input, &tmp_output, &output) {
        Ok(_) => println!("Successfully imported OSM data to {}", output.to_string_lossy()),
        Err(err) => {
            // Make a best-effort attempt to remove the unfinished mess
            // we may have potentially left behind, deliberately ignoring
            // the error.
            let _ = fs::remove_file(tmp_output);

            for cause in err.chain() {
                eprintln!("{}", cause);
            }
            std::process::exit(1);
        }
    }
}
