pub mod color;
pub mod parser;
mod style_cache;
pub mod styler;
pub mod token;

use crate::mapcss::token::InputPosition;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
enum MapcssError {
    LexerError {
        message: String,
        pos: InputPosition,
    },
    ParseError {
        message: String,
        pos: InputPosition,
        file_name: String,
    },
}

impl Error for MapcssError {}

impl fmt::Display for MapcssError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MapcssError::LexerError { message, pos } => {
                f.write_fmt(format_args!("lexer error: {} (at {})", message, pos))
            }
            MapcssError::ParseError {
                message,
                pos,
                file_name,
            } => f.write_fmt(format_args!("parse error: {} ({} at {})", message, file_name, pos)),
        }
    }
}
