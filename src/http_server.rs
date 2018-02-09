use errors::*;

use std::collections::HashSet;
use draw::drawer::Drawer;
use geodata::reader::GeodataReader;
use mapcss::parser::{Parser, Rule};
use mapcss::styler::Styler;
use mapcss::token::Tokenizer;
use num_cpus;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use std::thread;
use tile::Tile;

#[cfg_attr(feature = "cargo-clippy", allow(implicit_hasher))]
pub fn run_server(
    address: &str,
    geodata_file: &str,
    stylesheet_file: &str,
    osm_ids: Option<HashSet<u64>>,
) -> Result<()> {
    let rules = read_style(stylesheet_file)?;

    let server = Arc::new(HttpServer {
        styler: Styler::new(rules),
        reader: GeodataReader::new(geodata_file).chain_err(|| "Failed to load the geodata file")?,
        drawer: Drawer::new(),
        osm_ids
    });

    let thread_count = num_cpus::get();

    let mut senders: Vec<Sender<TcpStream>> = Vec::new();
    let mut receivers: Vec<Receiver<TcpStream>> = Vec::new();

    for _ in 0..thread_count {
        let (tx, rx) = mpsc::channel();
        senders.push(tx);
        receivers.push(rx);
    }

    let mut handlers = Vec::new();

    for receiver in receivers {
        let server_ref = Arc::clone(&server);
        handlers.push(thread::spawn(move || {
            while let Ok(stream) = receiver.recv() {
                server_ref.handle_connection(stream);
            }
        }));
    }

    let tcp_listener =
        TcpListener::bind(address).chain_err(|| format!("Failed to bind to {}", address))?;
    let mut thread_id = 0;

    for tcp_stream in tcp_listener.incoming() {
        if let Ok(stream) = tcp_stream {
            senders[thread_id].send(stream).unwrap();
            thread_id = (thread_id + 1) % senders.len();
        }
    }

    for h in handlers {
        h.join().unwrap();
    }

    Ok(())
}

struct HttpServer<'a> {
    styler: Styler,
    reader: GeodataReader<'a>,
    drawer: Drawer,
    osm_ids: Option<HashSet<u64>>,
}

impl<'a> HttpServer<'a> {
    fn handle_connection(&self, stream: TcpStream) {
        let peer_addr = stream.peer_addr();
        match self.try_handle_connection(stream) {
            Ok(_) => {}
            Err(e) => {
                let peer_addr_str = match peer_addr {
                    Ok(addr) => format!(" from {}", addr),
                    _ => String::new(),
                };
                eprintln!("Error processing request{}: {}", peer_addr_str, e)
            }
        }
    }

    fn try_handle_connection(&self, stream: TcpStream) -> Result<()> {
        let mut rdr = BufReader::new(stream);

        let first_line = match rdr.by_ref().lines().next() {
            Some(Ok(line)) => line,
            _ => bail!("Failed to read the first line from the TCP stream"),
        };

        let path = extract_path_from_request(&first_line)?;
        let tile = match extract_tile_from_path(&path) {
            Some(tile) => tile,
            _ => bail!("<{}> doesn't look like a valid tile ID", path),
        };

        let entities = self.reader.get_entities_in_tile(&tile, &self.osm_ids);
        let tile_png_bytes = self.drawer.draw_tile(&entities, &tile, &self.styler).unwrap();

        let header = [
            "HTTP/1.1 200 OK",
            "Content-Type: image/png",
            &format!("Content-Length: {}", tile_png_bytes.len()),
            "Connection: close",
            "",
            "",
        ].join("\r\n");

        let mut output_stream = rdr.into_inner();

        // Errors at this stage usually happen when the user scrolls the map and the outstanding
        // requests get terminated. We're not interested in reporting these errors, but there's no
        // point in continuing after a write fails either.
        if output_stream.write_all(header.as_bytes()).is_ok() {
            let _ = output_stream.write_all(&tile_png_bytes);
        }

        Ok(())
    }
}

fn extract_path_from_request(first_line: &str) -> Result<String> {
    let tokens: Vec<_> = first_line.split(' ').collect();
    if tokens.len() != 3 {
        bail!("<{}> doesn't look like a valid HTTP request", first_line);
    }
    let method = tokens[0];
    if method != "GET" {
        bail!("Invalid HTTP method: {}", method);
    }
    let http_version = tokens[2];
    if http_version != "HTTP/1.1" && http_version != "HTTP/1.0" {
        bail!("Invalid HTTP version: {}", http_version);
    }
    Ok(tokens[1].to_string())
}

fn read_style(stylesheet_file: &str) -> Result<Vec<Rule>> {
    let mut stylesheet_reader =
        File::open(stylesheet_file).chain_err(|| "Failed to open the stylesheet file")?;
    let mut stylesheet = String::new();
    stylesheet_reader
        .read_to_string(&mut stylesheet)
        .chain_err(|| "Failed to read the stylesheet file")?;
    Parser::new(Tokenizer::new(&stylesheet))
        .parse()
        .chain_err(|| "Failed to parse the stylesheet file")
}

fn extract_tile_from_path(path: &str) -> Option<Tile> {
    let expected_token_count = 3;

    let mut tokens = path.trim_right_matches(".png")
        .rsplit('/')
        .take(expected_token_count)
        .collect::<Vec<_>>();

    if tokens.len() != expected_token_count {
        return None;
    }

    tokens.reverse();
    let (z_str, x_str, y_str) = (tokens[0], tokens[1], tokens[2]);

    match (z_str.parse(), x_str.parse(), y_str.parse()) {
        (Ok(z), Ok(x), Ok(y)) => Some(Tile { zoom: z, x, y }),
        _ => None,
    }
}
