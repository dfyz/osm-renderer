pub mod color;
pub mod parser;
mod style_cache;
pub mod styler;
pub mod token;

mod errors {
    use crate::mapcss::token::InputPosition;
    use error_chain::*;

    error_chain! {
        errors {
            LexerError(message: String, pos: InputPosition) {
                description("lexer error"),
                display("lexer error: {} (at {})", message, pos),
            }
            ParseError(message: String, pos: InputPosition, file_name: String) {
                description("parse error"),
                display("parse error: {} ({} at {})", message, file_name, pos),
            }
        }
    }
}
