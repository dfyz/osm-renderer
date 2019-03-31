pub mod color;
pub mod parser;
mod style_cache;
pub mod styler;
pub mod token;

use crate::mapcss::token::InputPosition;
use failure::{Backtrace, Fail};
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

impl Fail for MapcssError {
    fn name(&self) -> Option<&str> {
        Some("renderer::mapcss::MapcssError")
    }

    fn cause(&self) -> Option<&dyn Fail> {
        None
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        None
    }
}

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
