mod grammar;
mod selector;

#[cfg(test)]
mod tests {
    use mapcss::grammar::parse_Selector;
    use mapcss::selector::*;

    #[test]
    fn test_simple_selectors() {
        assert_eq!(parse_Selector("node"), Ok(Selector {
            object_type: ObjectType::Node,
            min_zoom: None,
            max_zoom: None,
        }));
        assert_eq!(parse_Selector("way"), Ok(Selector {
            object_type: ObjectType::Way(WayType::All),
            min_zoom: None,
            max_zoom: None,
        }));
        assert!(parse_Selector("object").is_err());
    }

    #[test]
    fn test_zoom_selectors() {
        assert_eq!(parse_Selector("area|z11-13"), Ok(Selector {
            object_type: ObjectType::Way(WayType::Area),
            min_zoom: Some(11),
            max_zoom: Some(13),
        }));
        assert_eq!(parse_Selector("way|z15-"), Ok(Selector {
            object_type: ObjectType::Way(WayType::All),
            min_zoom: Some(15),
            max_zoom: None,
        }));
        assert_eq!(parse_Selector("way|z-3"), Ok(Selector {
            object_type: ObjectType::Way(WayType::All),
            min_zoom: None,
            max_zoom: Some(3),
        }));
        assert!(parse_Selector("way|z-").is_err());
        assert!(parse_Selector("node|z").is_err());
        assert!(parse_Selector("way|z12-123456").is_err());
    }
}
