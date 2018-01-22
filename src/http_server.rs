use errors::*;

use std::collections::HashSet;
use draw::drawer::Drawer;
use futures;
use futures::future::FutureResult;
use geodata::reader::GeodataReader;
use hyper;
use hyper::{Get, StatusCode};
use hyper::header::ContentType;
use hyper::server::{Http, Request, Response, Service};
use mapcss::parser::Parser;
use mapcss::styler::Styler;
use mapcss::token::Tokenizer;
use std::fs::File;
use std::io::Read;
use tile::Tile;

#[cfg_attr(feature = "cargo-clippy", allow(implicit_hasher))]
pub fn run_server(
    address: &str,
    geodata_file: &str,
    stylesheet_file: &str,
    osm_ids: Option<HashSet<u64>>,
) -> Result<()> {
    let mut stylesheet_reader =
        File::open(stylesheet_file).chain_err(|| "Failed to open the stylesheet file")?;
    let mut stylesheet = String::new();
    stylesheet_reader
        .read_to_string(&mut stylesheet)
        .chain_err(|| "Failed to read the stylesheet file")?;
    let mut parser = Parser::new(Tokenizer::new(&stylesheet));

    let reader = GeodataReader::new(geodata_file).chain_err(|| "Failed to load the geodata file")?;
    let rules = parser
        .parse()
        .chain_err(|| "Failed to parse the stylesheet file")?;

    let tile_server = TileServer {
        reader,
        styler: Styler::new(rules),
        drawer: Drawer::new(),
        osm_ids: osm_ids,
    };

    let addr = address
        .parse()
        .chain_err(|| format!("Failed to parse {} as server endpoint", address))?;
    let create_handler = move || {
        Ok(TileHandler {
            tile_server: &tile_server,
        })
    };
    let server = Http::new()
        .bind(&addr, create_handler)
        .chain_err(|| "Failed to spawn the HTTP server")?;
    server.run().chain_err(|| "Failed to run the HTTP server")
}

struct TileServer<'a> {
    reader: GeodataReader<'a>,
    styler: Styler,
    drawer: Drawer,
    osm_ids: Option<HashSet<u64>>,
}

struct TileHandler<'a> {
    tile_server: &'a TileServer<'a>,
}

impl<'a> Service for TileHandler<'a> {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = FutureResult<Self::Response, hyper::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let tile = match extract_tile_from_request(&req) {
            None => {
                return futures::future::ok(
                    Response::new()
                        .with_status(StatusCode::BadRequest)
                        .with_body("Invalid tile request"),
                );
            }
            Some(tile) => tile,
        };

        let response = match self.draw_tile_contents(&tile) {
            Ok(content) => Response::new()
                .with_header(ContentType::png())
                .with_body(content),
            Err(e) => Response::new()
                .with_status(StatusCode::InternalServerError)
                .with_body(format!("{}", e)),
        };

        futures::future::ok(response)
    }
}

impl<'a> TileHandler<'a> {
    fn draw_tile_contents(&self, tile: &Tile) -> Result<Vec<u8>> {
        let entities = self.tile_server
            .reader
            .get_entities_in_tile(tile, &self.tile_server.osm_ids);
        let tile_png_bytes = self.tile_server.drawer.draw_tile(&entities, tile, &self.tile_server.styler)?;
        Ok(tile_png_bytes)
    }
}

fn extract_tile_from_request(req: &Request) -> Option<Tile> {
    match *req.method() {
        Get => {
            let expected_token_count = 3;

            let mut tokens = req.uri()
                .path()
                .trim_right_matches(".png")
                .rsplit('/')
                .take(expected_token_count)
                .collect::<Vec<_>>();

            if tokens.len() != expected_token_count {
                return None;
            }

            tokens.reverse();
            let (z_str, x_str, y_str) = (tokens[0], tokens[1], tokens[2]);

            match (z_str.parse(), x_str.parse(), y_str.parse()) {
                (Ok(z), Ok(x), Ok(y)) => Some(Tile {
                    zoom: z,
                    x: x,
                    y: y,
                }),
                _ => None,
            }
        }
        _ => None,
    }
}
