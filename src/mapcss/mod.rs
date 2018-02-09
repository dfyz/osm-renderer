pub mod ast;
pub mod color;
pub mod parser;
pub mod styler;
pub mod token;

use mapcss::token::InputPosition;

mod errors {
    error_chain! {
        errors {
            LexerError(message: String, pos: InputPosition) {
                description("lexer error"),
                display("lexer error: {} (at {})", message, pos),
            }
            ParseError(message: String, pos: InputPosition) {
                description("parse error"),
                display("parse error: {} (at {})", message, pos),
            }
        }
    }
}
