use lalrpop_util::ParseError;

mod ast;
mod grammar;
mod token;

pub type ParsingResult<'a, T> = Result<T, ParseError<usize, token::Tok<'a>, String>>;

pub fn parse_selector(input: &str) -> ParsingResult<ast::Selector> {
    let lexer = token::Tokenizer::new(input);
    grammar::parse_Selector(input, lexer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapcss::ast::*;

    #[test]
    fn test_simple_selectors() {
        assert_eq!(parse_selector("node"), Ok(Selector {
            object_type: ObjectType::Node,
            min_zoom: None,
            max_zoom: None,
        }));
        assert_eq!(parse_selector("way"), Ok(Selector {
            object_type: ObjectType::Way(WayType::All),
            min_zoom: None,
            max_zoom: None,
        }));
        assert!(parse_selector("object").is_err());
    }

    #[test]
    fn test_zoom_selectors() {
        assert_eq!(parse_selector("area|z11-13"), Ok(Selector {
            object_type: ObjectType::Way(WayType::Area),
            min_zoom: Some(11),
            max_zoom: Some(13),
        }));
        assert_eq!(parse_selector("way|z15-"), Ok(Selector {
            object_type: ObjectType::Way(WayType::All),
            min_zoom: Some(15),
            max_zoom: None,
        }));
        assert_eq!(parse_selector("way|z-3"), Ok(Selector {
            object_type: ObjectType::Way(WayType::All),
            min_zoom: None,
            max_zoom: Some(3),
        }));
        assert!(parse_selector("way|z-").is_err());
        assert!(parse_selector("node|z").is_err());
        assert!(parse_selector("way|z12-123456").is_err());
    }
}
