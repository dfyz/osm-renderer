pub mod color;
pub mod parser;
mod style_cache;
pub mod styler;
pub mod token;

use crate::mapcss::token::InputPosition;
use failure::Fail;

#[derive(Debug, Fail)]
enum MapcssError {
    #[fail(display = "lexer error: {} (at {})", message, pos)]
    LexerError { message: String, pos: InputPosition },
    #[fail(display = "parse error: {} ({} at {})", message, file_name, pos)]
    ParseError {
        message: String,
        pos: InputPosition,
        file_name: String,
    },
}
