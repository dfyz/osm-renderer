use crate::draw::drawer::Drawer;
use crate::draw::tile_pixels::TilePixels;
use crate::geodata::reader::GeodataReader;
use crate::mapcss::parser::parse_file;
use crate::mapcss::styler::{StyleType, Styler};
use crate::perf_stats::PerfStats;
use crate::tile::{Tile, MAX_ZOOM};
use failure::{bail, format_err, Error, ResultExt};
use num_cpus;
use std::collections::HashSet;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

enum HandlerMessage {
    Terminate,
    ServeTile { path: String, stream: TcpStream },
}

struct HandlerState {
    current_scale: usize,
    current_pixels: Box<TilePixels>,
}

#[cfg_attr(feature = "cargo-clippy", allow(clippy::implicit_hasher))]
pub fn run_server(
    address: &str,
    geodata_file: &str,
    stylesheet_file: &str,
    stylesheet_type: &StyleType,
    font_size_multiplier: Option<f64>,
    osm_ids: Option<HashSet<u64>>,
) -> Result<(), Error> {
    let (base_path, file_name) = split_stylesheet_path(stylesheet_file)?;
    let rules = parse_file(&base_path, &file_name).context("Failed to parse the stylesheet file")?;

    let server = Arc::new(HttpServer {
        styler: Styler::new(rules, stylesheet_type, font_size_multiplier),
        reader: GeodataReader::load(geodata_file).context("Failed to load the geodata file")?,
        drawer: Drawer::new(&base_path),
        osm_ids,
        perf_stats: Mutex::new(PerfStats::default()),
    });

    let thread_count = num_cpus::get();

    let mut senders: Vec<Sender<HandlerMessage>> = Vec::new();
    let mut receivers: Vec<Receiver<HandlerMessage>> = Vec::new();

    for _ in 0..thread_count {
        let (tx, rx) = mpsc::channel();
        senders.push(tx);
        receivers.push(rx);
    }

    let mut handlers = Vec::new();

    for receiver in receivers {
        let server_ref = Arc::clone(&server);
        handlers.push(thread::spawn(move || {
            let initial_scale = 1;

            let mut handler_state = HandlerState {
                current_scale: initial_scale,
                current_pixels: Box::new(TilePixels::new(initial_scale)),
            };

            while let Ok(msg) = receiver.recv() {
                match msg {
                    HandlerMessage::Terminate => break,
                    HandlerMessage::ServeTile { path, stream } => {
                        server_ref.handle_connection(&path, stream, &mut handler_state)
                    }
                }
            }
        }));
    }

    let tcp_listener = TcpListener::bind(address).context(format!("Failed to bind to {}", address))?;
    let mut thread_id = 0;

    for tcp_stream in tcp_listener.incoming() {
        if let Ok(mut stream) = tcp_stream {
            let path = match extract_path_from_stream(&mut stream) {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("{} didn't send a valid HTTP request: {}", peer_addr(&stream), e);
                    continue;
                }
            };

            if path == "/shutdown" {
                eprintln!("Shutting down due to a shutdown request");
                for sender in senders {
                    sender.send(HandlerMessage::Terminate).unwrap();
                }
                break;
            }

            senders[thread_id]
                .send(HandlerMessage::ServeTile { path, stream })
                .unwrap();
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
    perf_stats: Mutex<PerfStats>,
}

impl<'a> HttpServer<'a> {
    fn handle_connection(&self, path: &str, mut stream: TcpStream, state: &mut HandlerState) {
        match self.try_handle_connection(path, &mut stream, state) {
            Ok(_) => {}
            Err(e) => eprintln!("Error processing request from {}: {}", peer_addr(&stream), e),
        }
    }

    fn try_handle_connection(&self, path: &str, stream: &mut TcpStream, state: &mut HandlerState) -> Result<(), Error> {
        if cfg!(feature = "perf-stats") && path == "/perf_stats" {
            let perf_stats_html = self.perf_stats.lock().unwrap().to_html();
            serve_data(stream, perf_stats_html.as_bytes(), "text/html");
            return Ok(());
        }

        let tile = match extract_tile_from_path(&path) {
            Some(tile) => tile,
            _ => bail!("<{}> doesn't look like a valid tile ID", path),
        };

        if cfg!(feature = "perf-stats") {
            crate::perf_stats::start_tile(tile.tile.zoom);
        }

        let entities = {
            let _m = crate::perf_stats::measure("Get tile entities");
            self.reader
                .get_entities_in_tile_with_neighbors(&tile.tile, &self.osm_ids)
        };

        if tile.scale != state.current_scale {
            let _m = crate::perf_stats::measure("Re-scaling TilePixels");
            state.current_scale = tile.scale;
            state.current_pixels = Box::new(TilePixels::new(tile.scale));
        }

        let tile_png_bytes = self
            .drawer
            .draw_tile(
                &entities,
                &tile.tile,
                &mut state.current_pixels,
                state.current_scale,
                &self.styler,
            )
            .unwrap();

        if cfg!(feature = "perf-stats") {
            crate::perf_stats::finish_tile(&mut self.perf_stats.lock().unwrap());
        }

        serve_data(stream, &tile_png_bytes, "image/png");

        Ok(())
    }
}

fn serve_data(stream: &mut TcpStream, data: &[u8], content_type: &str) {
    let header = [
        "HTTP/1.1 200 OK",
        &format!("Content-Type: {}", content_type),
        &format!("Content-Length: {}", data.len()),
        "Connection: close",
        "",
        "",
    ]
    .join("\r\n");

    // Errors at this stage usually happen when the outstanding requests get terminated for some
    // reason (e.g. the user scrolls the map). We're not interested in reporting these errors,
    // but there's no point in continuing after a write fails either.
    if stream.write_all(header.as_bytes()).is_ok() {
        let _ = stream.write_all(&data);
    }
}

fn extract_path_from_stream(stream: &mut TcpStream) -> Result<String, Error> {
    let mut rdr = BufReader::new(stream);
    let first_line = match rdr.by_ref().lines().next() {
        Some(Ok(line)) => line,
        _ => bail!("Failed to read the first line from the TCP stream"),
    };
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

struct RequestTile {
    tile: Tile,
    scale: usize,
}

fn extract_tile_from_path(path: &str) -> Option<RequestTile> {
    let expected_token_count = 3;

    let real_path = match path.rfind('?') {
        Some(pos) => &path[..pos],
        None => path,
    };

    let mut tokens = real_path
        .trim_end_matches(".png")
        .rsplit('/')
        .take(expected_token_count)
        .collect::<Vec<_>>();

    if tokens.len() != expected_token_count {
        return None;
    }

    tokens.reverse();
    let (z_str, x_str, mut y_str) = (tokens[0], tokens[1], tokens[2]);

    let mut scale = 1;

    let y_tokens = y_str.split('@').collect::<Vec<_>>();
    if y_tokens.len() == 2 {
        if let Ok(parsed_scale) = y_tokens[1].trim_end_matches('x').parse() {
            y_str = y_tokens[0];
            scale = parsed_scale;
        }
    }

    match (z_str.parse(), x_str.parse(), y_str.parse()) {
        (Ok(z), Ok(x), Ok(y)) if z <= MAX_ZOOM => Some(RequestTile {
            tile: Tile { zoom: z, x, y },
            scale,
        }),
        _ => None,
    }
}

fn split_stylesheet_path(file_path: &str) -> Result<(PathBuf, String), Error> {
    let mut result = PathBuf::from(file_path);
    let file_name = result
        .file_name()
        .and_then(|x| x.to_str().map(ToString::to_string))
        .ok_or_else(|| format_err!("Failed to extract the file name for {}", file_path))?;
    result.pop();
    Ok((result, file_name))
}

fn peer_addr(stream: &TcpStream) -> String {
    stream
        .peer_addr()
        .map(|x| format!("{}", x))
        .unwrap_or_else(|_| "N/A".to_string())
}
