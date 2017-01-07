use errors::*;

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read};

use capnp::message::{Allocator, Builder};
use geodata_capnp::{geodata,tag_list};
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
    let mut message = Builder::new_default();
    convert_to_message(&mut message, parsed_xml)?;

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
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::result::Result<(), ::std::fmt::Error> {
        write!(f, "<{}> at {}", self.name, self.input_position)
    }
}

struct OsmEntity {
    global_id: u64,
    initial_elem: OsmXmlElement,
    additional_elems: Vec<OsmXmlElement>,
}

impl OsmEntity {
    fn new(initial_element: OsmXmlElement) -> Option<OsmEntity> {
        initial_element
            .attr_map
            .get("id")
            .and_then(|x| x.parse().ok())
            .map(|id| OsmEntity {
                global_id: id,
                initial_elem: initial_element,
                additional_elems: vec![],
            })
    }

    fn get_elems_by_name<'a>(&'a self, name: &str) -> Vec<&'a OsmXmlElement> {
        self
            .additional_elems
            .iter()
            .filter(|x| x.name == name)
            .collect::<Vec<_>>()
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

    fn add(&mut self, entity: OsmEntity) {
        let old_size = self.entities.len();
        self.global_id_to_local_id.insert(entity.global_id, old_size);
        self.entities.push(entity);
    }

    fn translate_id(&self, global_id: u64) -> Result<usize> {
        match self.global_id_to_local_id.get(&global_id) {
            Some(value) => Ok(*value),
            None => bail!("Failed to find an entity with ID = {}", global_id),
        }
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
            entity.additional_elems.push(osm_elem);
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

fn get_required_attr(osm_elem: &OsmXmlElement, attr_name: &str) -> Result<String> {
    match osm_elem.attr_map.get(attr_name) {
        Some(value) => Ok(value.clone()),
        None => bail!("Element {} doesn't have required attribute: {}", osm_elem, attr_name),
    }
}

fn parse_required_attr<T>(osm_elem: &OsmXmlElement, attr_name: &str) -> Result<T>
    where
        T: ::std::str::FromStr,
        T::Err : ::std::error::Error + ::std::marker::Send + 'static
{
    let value = get_required_attr(osm_elem, attr_name)?;

    let parsed_value = value
        .parse::<T>()
        .chain_err(|| format!("Failed to parse the value of attribute {} for element {}", attr_name, osm_elem))?;

    Ok(parsed_value)
}

fn collect_tags(tag_builder: &mut tag_list::Builder, osm_entity: &OsmEntity) -> Result<()> {
    let tags_in = osm_entity.get_elems_by_name("tag");
    let mut tags_out = tag_builder.borrow().init_tags(tags_in.len() as u32);

    for (i, tag_in) in tags_in.iter().enumerate() {
        let mut tag_out = tags_out.borrow().get(i as u32);
        tag_out.set_key(&get_required_attr(&tag_in, "k")?);
        tag_out.set_value(&get_required_attr(&tag_in, "v")?);
    }

    Ok(())
}

fn convert_to_message<A: Allocator>(message: &mut Builder<A>, osm_xml: ParsedOsmXml) -> Result<()> {
    let mut geodata = message.init_root::<geodata::Builder>();

    {
        let mut nodes = geodata.borrow().init_nodes(osm_xml.node_storage.entities.len() as u32);
        for (i, node_in) in osm_xml.node_storage.entities.iter().enumerate() {
            let mut node_out = nodes.borrow().get(i as u32);

            node_out.set_global_id(node_in.global_id);

            {
                let mut coords = node_out.borrow().init_coords();

                coords.set_lat(parse_required_attr(&node_in.initial_elem, "lat")?);
                coords.set_lon(parse_required_attr(&node_in.initial_elem, "lon")?);
            }

            collect_tags(&mut node_out.init_tags(), &node_in)?;
        }
    }

    {
        let mut ways = geodata.borrow().init_ways(osm_xml.way_storage.entities.len() as u32);
        for (i, way_in) in osm_xml.way_storage.entities.iter().enumerate() {
            let mut way_out = ways.borrow().get(i as u32);

            way_out.set_global_id(way_in.global_id);

            {
                let nds_in = way_in.get_elems_by_name("nd");
                let mut nds_out = way_out.borrow().init_local_node_ids(nds_in.len() as u32);
                for (j, nd_in) in nds_in.iter().enumerate() {
                    let local_node_id = osm_xml.node_storage.translate_id(parse_required_attr(&nd_in, "ref")?)?;
                    nds_out.set(j as u32, local_node_id as u32);
                }
            }

            collect_tags(&mut way_out.init_tags(), &way_in)?;
        }
    }

    {
        let mut relations = geodata.borrow().init_relations(osm_xml.relation_storage.entities.len() as u32);
        for (i, rel_in) in osm_xml.relation_storage.entities.iter().enumerate() {
            let mut rel_out = relations.borrow().get(i as u32);

            rel_out.set_global_id(rel_in.global_id);

            {
                let members = rel_in.get_elems_by_name("member");

                let node_members_in = members
                    .iter()
                    .filter(|x| x.attr_map.get("type") == Some(&"node".to_string()))
                    .collect::<Vec<_>>();
                {
                    let mut node_members_out = rel_out.borrow().init_local_node_ids(node_members_in.len() as u32);
                    for (j, node_member_in) in node_members_in.iter().enumerate() {
                        let local_node_id = osm_xml.node_storage.translate_id(parse_required_attr(&node_member_in, "ref")?)?;
                        node_members_out.set(j as u32, local_node_id as u32);
                    }
                }

                let way_members_in = members
                    .iter()
                    .filter(|x| x.attr_map.get("type") == Some(&"way".to_string()))
                    .collect::<Vec<_>>();
                {
                    let mut way_members_out = rel_out.borrow().init_local_way_ids(way_members_in.len() as u32);
                    for (j, way_member_in) in way_members_in.iter().enumerate() {
                        let local_way_id = osm_xml.way_storage.translate_id(parse_required_attr(&way_member_in, "ref")?)?;
                        way_members_out.set(j as u32, local_way_id as u32);
                    }
                }
            }

            collect_tags(&mut rel_out.init_tags(), &rel_in)?;
        }
    }

    Ok(())
}