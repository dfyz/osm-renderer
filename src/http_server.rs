use errors::*;

use geodata::reader::GeodataReader;
use hyper::header::{ContentLength, ContentType};
use hyper::method::Method;
use hyper::server::{Handler, Listening, Response, Request, Server};
use hyper::status::StatusCode;
use hyper::uri::RequestUri;
use std::io::Write;
use tile::Tile;

pub fn run_server(address: &str, geodata_file: &str) -> Result<Listening> {
    let reader = GeodataReader::new(geodata_file).chain_err(|| "Failed to load the geodata file")?;
    let handler = TileServer { reader: reader };
    let server = Server::http(address).chain_err(|| "Failed to spawn the HTTP server")?;
    server.handle(handler).chain_err(|| "Failed to run the HTTP server")
}

struct TileServer<'a> {
    reader: GeodataReader<'a>,
}

impl<'a> Handler for TileServer<'a> {
    fn handle(&self, req: Request, mut resp: Response) {
        let tile = extract_tile_from_request(&req);
        if tile.is_none() {
            *resp.status_mut() = StatusCode::BadRequest;
            write_bytes_to_response(resp, "Invalid tile request".as_bytes());
            return;
        }

        let tile = tile.unwrap();
        info!("Processing tile ({}, {}), z={}", tile.x, tile.y, tile.zoom);

        match self.draw_tile_contents(&tile) {
            Ok(content) => {
                *resp.status_mut() = StatusCode::Ok;
                resp.headers_mut().set(ContentType::png());
                write_bytes_to_response(resp, content);
            },
            Err(e) => {
                *resp.status_mut() = StatusCode::InternalServerError;
                let err_msg = format!("{}", e);
                write_bytes_to_response(resp, err_msg.as_bytes());
            }
        }
    }
}

impl<'a> TileServer<'a> {
    fn draw_tile_contents(&self, tile: &Tile) -> Result<&[u8]> {
        Ok(&[])
    }
}

fn extract_tile_from_request(req: &Request) -> Option<Tile> {
    match (&req.method, &req.uri) {
        (&Method::Get, &RequestUri::AbsolutePath(ref uri)) => {
            let expected_token_count = 3;

            let mut tokens = uri
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
                (Ok(z), Ok(x), Ok(y)) => {
                    Some(Tile {
                        zoom: z,
                        x: x,
                        y: y,
                    })
                },
                _ => None,
            }
        },
        _ => None,
    }
}

fn write_bytes_to_response(mut resp: Response, bytes: &[u8]) {
    resp.headers_mut().set(ContentLength(bytes.len() as u64));
    let res = resp.start().map(|mut x| x.write_all(bytes));
    match res {
        Err(e) => error!("Error while forming HTTP response: {}", e),
        _ => {},
    }
}
