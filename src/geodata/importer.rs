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
        for a in attrs.into_iter() {
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
        let maybe_id = initial_element.attr_map.get("id").map(|x| x.parse());

        match maybe_id {
            Some(Ok(parsed_id)) => Ok(OsmEntity {
                global_id: parsed_id,
                osm_type: initial_element.name.clone(),
                elems: vec![initial_element],
            }),
            _ => Err(OsmParsingError::from_reason(
                format!("Element {} doesn't have a numeric id attribute", initial_element.name)
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
        },
        None => {
            parsing_state.current_entity = Some(
                OsmEntity::from_initial_element(osm_elem)?
            );
        },
    }
    Ok(())
}

fn process_end_element(name: OwnedName, parsing_state: &mut ParsingState) {
    let is_final_entity_element =
        if let Some(ref entity) = parsing_state.current_entity {
            entity.osm_type == name.local_name
        } else {
            false
        };

    if !is_final_entity_element {
        return
    }

    let entity = parsing_state.current_entity.take().unwrap();

    let maybe_storage = match entity.osm_type.as_ref() {
        "node" => Some(&mut parsing_state.node_storage),
        "way" => Some(&mut parsing_state.way_storage),
        "relation" => Some(&mut parsing_state.relation_storage),
        _ => None,
    };

    if let Some(storage) = maybe_storage {
        storage.add(entity);
    }
}
