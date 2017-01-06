use errors::*;

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read};

use capnp::message::{HeapAllocator, Builder};
use geodata_capnp::geodata;
use xml::attribute::OwnedAttribute;
use xml::common::{Position,TextPosition};
use xml::name::OwnedName;
use xml::reader::{EventReader, XmlEvent};

pub fn import(input: &str, output: &str) -> Result<()> {
    let input_file = File::open(input).chain_err(|| format!("Failed to open {} for reading", input))?;
    let output_file = File::create(output).chain_err(|| format!("Failed to open {} for writing", output))?;

    let parser = EventReader::new(BufReader::new(input_file));
    let mut writer = BufWriter::new(output_file);

    info!("Parsing XML");
    let parsed_xml = parse_osm_xml(parser)?;

    info!("Converting geodata to internal format");
    let message = convert_to_message(parsed_xml)?;

    ::capnp::serialize_packed::write_message(&mut writer, &message)
        .chain_err(|| "Failed to write the imported data to the output file")?;
    Ok(())
}

struct OsmXmlElement {
    name: String,
    attr_map: HashMap<String, String>,
    input_position: TextPosition,
}

impl OsmXmlElement {
    fn new(name: OwnedName, attrs: Vec<OwnedAttribute>, input_position: TextPosition) -> OsmXmlElement {
        let mut attr_map = HashMap::new();
        for a in attrs.into_iter() {
            attr_map.insert(a.name.local_name, a.value);
        }
        OsmXmlElement {
            name: name.local_name,
            attr_map: attr_map,
            input_position: input_position
        }
    }
}

impl ::std::fmt::Display for OsmXmlElement {
    fn fmt(self: &OsmXmlElement, f: &mut ::std::fmt::Formatter) -> ::std::result::Result<(), ::std::fmt::Error> {
        write!(f, "<{}> at {}", self.name, self.input_position)
    }
}

struct OsmEntity {
    global_id: u64,
    elems: Vec<OsmXmlElement>,
}

impl OsmEntity {
    fn new(initial_element: OsmXmlElement) -> Option<OsmEntity> {
        initial_element
            .attr_map
            .get("id")
            .and_then(|x| x.parse().ok())
            .map(|id| OsmEntity {
                global_id: id,
                elems: vec![initial_element],
            })
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

struct ParsedOsmXml {
    node_storage: OsmEntityStorage,
    way_storage: OsmEntityStorage,
    relation_storage: OsmEntityStorage,

    current_entity_with_type: Option<(OsmEntity, String)>,
}

fn parse_osm_xml<R: Read>(mut parser: EventReader<R>) -> Result<ParsedOsmXml> {
    let mut parsing_state = ParsedOsmXml {
        node_storage: OsmEntityStorage::new(),
        way_storage: OsmEntityStorage::new(),
        relation_storage: OsmEntityStorage::new(),
        current_entity_with_type: None,
    };

    loop {
        let e = parser.next().chain_err(|| "Failed to parse the input file")?;
        match e {
            XmlEvent::EndDocument => break,
            XmlEvent::StartElement {name, attributes, ..} => {
                process_start_element(name, attributes, parser.position(), &mut parsing_state)
            },
            XmlEvent::EndElement {name} => {
                process_end_element(name, &mut parsing_state);
            },
            _ => {}
        }
    }

    Ok(parsing_state)
}

fn process_start_element(
    name: OwnedName,
    attrs: Vec<OwnedAttribute>,
    input_position: TextPosition,
    parsing_state: &mut ParsedOsmXml
)
{
    let entity_type = name.local_name.clone();
    let osm_elem = OsmXmlElement::new(name, attrs, input_position);
    match parsing_state.current_entity_with_type {
        Some((ref mut entity, _)) => {
            entity.elems.push(osm_elem);
        },
        None => {
            let new_entity = OsmEntity::new(osm_elem);
            if new_entity.is_some() {
                parsing_state.current_entity_with_type = new_entity.map(|x| (x, entity_type));
            }
        },
    }
}

fn process_end_element(name: OwnedName, parsing_state: &mut ParsedOsmXml) {
    let is_final_entity_element =
        if let Some((_, ref entity_type)) = parsing_state.current_entity_with_type {
            *entity_type == name.local_name
        } else {
            false
        };

    if !is_final_entity_element {
        return
    }

    let (entity, entity_type) = parsing_state.current_entity_with_type.take().unwrap();

    let maybe_storage =
        match entity_type.as_ref() {
            "node" => Some(&mut parsing_state.node_storage),
            "way" => Some(&mut parsing_state.way_storage),
            "relation" => Some(&mut parsing_state.relation_storage),
            _ => None,
        };

    if let Some(storage) = maybe_storage {
        storage.add(entity);
    }
}

fn convert_to_message(osm_xml: ParsedOsmXml) -> Result<Builder<HeapAllocator>> {
    let mut message = Builder::new_default();
    {
        let geodata = message.init_root::<geodata::Builder>();

        let mut nodes = geodata.init_nodes(osm_xml.node_storage.entities.len() as u32);

        for (i, node_in) in osm_xml.node_storage.entities.iter().enumerate() {
            let mut node_out = nodes.borrow().get(i as u32);

            node_out.set_global_id(node_in.global_id);
        }
    }
    Ok(message)
}