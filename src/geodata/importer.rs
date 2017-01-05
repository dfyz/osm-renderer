use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read};

use capnp::message::{HeapAllocator, Builder};
use geodata_capnp::geodata;
use xml::attribute::OwnedAttribute;
use xml::name::OwnedName;
use xml::reader::{EventReader, XmlEvent};

pub fn import(input: &str, output: &str) -> Result<(), Box<Error>> {
    let parser = EventReader::new(BufReader::new(File::open(input)?));
    let mut writer = BufWriter::new(File::create(output)?);

    let message = read_geodata(parser)?;

    ::capnp::serialize_packed::write_message(&mut writer, &message)?;
    Ok(())
}

#[derive(Debug)]
pub struct OsmParsingError {
    reason: String,
}

impl fmt::Display for OsmParsingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "OSM parsing error: {}", self.reason)
    }
}

impl OsmParsingError {
    fn from_reason(reason: String) -> Box<OsmParsingError> {
        Box::new(OsmParsingError {
            reason: reason,
        })
    }
}

impl Error for OsmParsingError {
    fn description(self: &OsmParsingError) -> &str {
        &self.reason
    }
}

struct OsmXmlElement {
    name: String,
    attr_map: HashMap<String, String>,
}

impl OsmXmlElement {
    fn new(name: OwnedName, attrs: Vec<OwnedAttribute>) -> OsmXmlElement {
        let mut attr_map = HashMap::new();
        for a in attrs.iter() {
            attr_map.insert(a.name.local_name, a.value);
        }
        OsmXmlElement {
            name: name.local_name,
            attr_map: attr_map,
        }
    }
}

struct OsmEntity {
    global_id: u64,
    osm_type: String,
    elems: Vec<OsmXmlElement>,
}

impl OsmEntity {
    fn from_initial_element(initial_element: OsmXmlElement) -> Result<OsmEntity, Box<Error>> {
        match initial_element.attr_map.get("id") {
            Some(value) => Ok(OsmEntity {
                global_id: value.parse()?,
                osm_type: initial_element.name,
                elems: vec![initial_element],
            }),
            None => Err(OsmParsingError::from_reason(
                format!("Element {} doesn't have an id attribute", initial_element.name)
            ))
        }
    }
}

struct OsmEntityStorage {
    global_id_to_local_id: HashMap<u64, usize>,
    entities: Vec<OsmEntity>,
}

impl OsmEntityStorage {
    fn new() -> OsmEntityStorage {
        OsmEntityStorage {
            global_id_to_local_id: HashMap::new(),
            entities: Vec::new(),
        }
    }

    fn add(self: &mut OsmEntityStorage, entity: OsmEntity) {
        let old_size = self.entities.len();
        self.global_id_to_local_id.insert(entity.global_id, old_size);
        self.entities.push(entity);
    }
}

struct ParsingState {
    node_storage: OsmEntityStorage,
    way_storage: OsmEntityStorage,
    relation_storage: OsmEntityStorage,

    current_entity: Option<OsmEntity>,
}

fn read_geodata<R: Read>(parser: EventReader<R>) -> Result<Builder<HeapAllocator>, Box<Error>> {
    let mut message = Builder::new_default();
    let mut parsing_state = ParsingState {
        node_storage: OsmEntityStorage::new(),
        way_storage: OsmEntityStorage::new(),
        relation_storage: OsmEntityStorage::new(),
        current_entity: None,
    };

    {
        let mut geodata = message.init_root::<geodata::Builder>();

        for ev in parser {
            let e = ev?;
            match e {
                XmlEvent::StartElement {name, attributes, ..} => {
                    process_start_element(name, attributes, &mut parsing_state)?
                },
                XmlEvent::EndElement {name} => {
                    process_end_element(name, &mut parsing_state);
                },
                _ => {}
            }
        }
    }

    Ok(message)
}

fn process_start_element(name: OwnedName, attrs: Vec<OwnedAttribute>, parsing_state: &mut ParsingState) -> Result<(), Box<Error>> {
    let osm_elem = OsmXmlElement::new(name, attrs);
    match parsing_state.current_entity {
        Some(ref mut entity) => {
            entity.elems.push(osm_elem);
            Ok(())
        },
        None => {
            parsing_state.current_entity = Some(
                OsmEntity::from_initial_element(osm_elem)?
            );
            Ok(())
        },
    }
}

fn process_end_element<'a>(name: OwnedName, parsing_state: &'a mut ParsingState) {
    let get_storage_by_type = |state: &'a mut ParsingState, osm_type| {
        match osm_type {
            "node" => Some(&mut state.node_storage),
            "way" => Some(&mut state.way_storage),
            "relation" => Some(&mut state.relation_storage),
            _ => None,
        }
    };
    if let Some(entity) = parsing_state.current_entity {
        if name.local_name == entity.osm_type {
            if let Some(storage) = get_storage_by_type(parsing_state, &entity.osm_type) {
                storage.add(entity);
                parsing_state.current_entity = None;
            }
        }
    }
}
