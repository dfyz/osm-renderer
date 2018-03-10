pub mod ast;
pub mod color;
pub mod parser;
pub mod styler;
mod style;
pub mod token;

mod errors {
    use mapcss::token::InputPosition;

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
