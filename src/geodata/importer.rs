use errors::*;

use coords;
use tile;

use std::cmp::{max, min};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};

use xml::attribute::OwnedAttribute;
use xml::common::{Position, TextPosition};
use xml::name::OwnedName;
use xml::reader::{EventReader, XmlEvent};

pub fn import(input: &str, output: &str) -> Result<()> {
    let input_file =
        File::open(input).chain_err(|| format!("Failed to open {} for reading", input))?;
    let output_file =
        File::create(output).chain_err(|| format!("Failed to open {} for writing", output))?;

    let parser = EventReader::new(BufReader::new(input_file));
    let mut writer = BufWriter::new(output_file);

    println!("Parsing XML");
    let parsed_xml = parse_osm_xml(parser)?;

    println!("Converting geodata to internal format");
    save_to_internal_format(&mut writer, &parsed_xml)
        .chain_err(|| "Failed to write the imported data to the output file")?;
    Ok(())
}

struct OsmXmlElement {
    name: String,
    attrs: Vec<(String, String)>,
    input_position: TextPosition,
}

impl OsmXmlElement {
    fn new(
        name: OwnedName,
        attrs: Vec<OwnedAttribute>,
        input_position: TextPosition,
    ) -> OsmXmlElement {
        let mut attrs = attrs
            .into_iter()
            .map(|x| (x.name.local_name, x.value))
            .collect::<Vec<_>>();
        attrs.sort();
        OsmXmlElement {
            name: name.local_name,
            attrs: attrs,
            input_position: input_position,
        }
    }

    fn get_attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .binary_search_by(|probe| {
                let probe_str: &str = probe.0.as_ref();
                probe_str.cmp(name)
            })
            .ok()
            .map(|idx| self.attrs[idx].1.as_str())
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
            .get_attr("id")
            .and_then(|x| x.parse().ok())
            .map(|id| OsmEntity {
                global_id: id,
                initial_elem: initial_element,
                additional_elems: vec![],
            })
    }

    fn get_elems_by_name<'a>(
        &'a self,
        name: &'static str,
    ) -> Box<Iterator<Item = &'a OsmXmlElement> + 'a> {
        Box::new(self.additional_elems.iter().filter(move |x| x.name == name))
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
        self.global_id_to_local_id
            .insert(entity.global_id, old_size);
        self.entities.push(entity);
    }

    fn translate_id(&self, global_id: u64) -> Option<usize> {
        self.global_id_to_local_id.get(&global_id).cloned()
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

    let mut elem_count = 0;
    loop {
        let e = parser
            .next()
            .chain_err(|| "Failed to parse the input file")?;
        match e {
            XmlEvent::EndDocument => break,
            XmlEvent::StartElement {
                name, attributes, ..
            } => {
                process_start_element(name, attributes, parser.position(), &mut parsing_state);
                elem_count += 1;
                if elem_count % 100_000 == 0 {
                    println!(
                        "Got {} nodes, {} ways and {} relations",
                        parsing_state.node_storage.entities.len(),
                        parsing_state.way_storage.entities.len(),
                        parsing_state.relation_storage.entities.len()
                    );
                }
            }
            XmlEvent::EndElement { name } => {
                process_end_element(&name, &mut parsing_state);
            }
            _ => {}
        }
    }

    Ok(parsing_state)
}

fn process_start_element(
    name: OwnedName,
    attrs: Vec<OwnedAttribute>,
    input_position: TextPosition,
    parsing_state: &mut ParsedOsmXml,
) {
    let entity_type = name.local_name.clone();
    let osm_elem = OsmXmlElement::new(name, attrs, input_position);
    match parsing_state.current_entity_with_type {
        Some((ref mut entity, _)) => {
            entity.additional_elems.push(osm_elem);
        }
        None => {
            if let Some(new_entity) = OsmEntity::new(osm_elem) {
                parsing_state.current_entity_with_type = Some((new_entity, entity_type));
            }
        }
    }
}

fn process_end_element(name: &OwnedName, parsing_state: &mut ParsedOsmXml) {
    match parsing_state.current_entity_with_type {
        Some((_, ref entity_type)) if *entity_type == name.local_name => {}
        _ => return,
    }

    match parsing_state.current_entity_with_type.take() {
        Some((entity, entity_type)) => {
            let maybe_storage = match entity_type.as_ref() {
                "node" => Some(&mut parsing_state.node_storage),
                "way" => Some(&mut parsing_state.way_storage),
                "relation" => Some(&mut parsing_state.relation_storage),
                _ => None,
            };

            if let Some(storage) = maybe_storage {
                storage.add(entity);
            }
        }
        _ => {}
    }
}

fn get_required_attr<'a>(osm_elem: &'a OsmXmlElement, attr_name: &'static str) -> Result<&'a str> {
    match osm_elem.get_attr(attr_name) {
        Some(value) => Ok(value),
        None => bail!(
            "Element {} doesn't have required attribute: {}",
            osm_elem,
            attr_name
        ),
    }
}

fn parse_required_attr<T>(osm_elem: &OsmXmlElement, attr_name: &'static str) -> Result<T>
where
    T: ::std::str::FromStr,
    T::Err: ::std::error::Error + ::std::marker::Send + 'static,
{
    let value = get_required_attr(osm_elem, attr_name)?;

    let parsed_value = value.parse::<T>().chain_err(|| {
        format!(
            "Failed to parse the value of attribute {} for element {}",
            attr_name, osm_elem
        )
    })?;

    Ok(parsed_value)
}

type RawRefs = Vec<usize>;

fn collect_references<'a, I>(elems: I, storage: &OsmEntityStorage) -> RawRefs
where
    I: Iterator<Item = &'a OsmXmlElement>,
{
    elems
        .filter_map(|x| {
            x.get_attr("ref")
                .and_then(|y| y.parse().ok())
                .and_then(|y| storage.translate_id(y))
        })
        .collect::<Vec<_>>()
}

type RawTags = Vec<(String, String)>;

fn collect_tags(osm_entity: &OsmEntity, all_strings: &mut BTreeSet<String>) -> RawTags {
    let mut result = osm_entity
        .get_elems_by_name("tag")
        .filter_map(
            |x| match (get_required_attr(x, "k"), get_required_attr(x, "v")) {
                (Ok(k), Ok(v)) => {
                    all_strings.insert(k.to_string());
                    all_strings.insert(v.to_string());
                    Some((k.to_string(), v.to_string()))
                }
                _ => None,
            },
        )
        .collect::<Vec<_>>();

    result.sort();
    result
}

struct RawNode {
    global_id: u64,
    lat: f64,
    lon: f64,
    tags: RawTags,
}

impl coords::Coords for RawNode {
    fn lat(&self) -> f64 {
        self.lat
    }

    fn lon(&self) -> f64 {
        self.lon
    }
}

struct RawWay {
    global_id: u64,
    node_ids: RawRefs,
    tags: RawTags,
}

struct RawRelation {
    global_id: u64,
    way_ids: RawRefs,
    tags: RawTags,
}

#[derive(Default)]
struct TileReferences {
    local_node_ids: BTreeSet<usize>,
    local_way_ids: BTreeSet<usize>,
    local_relation_ids: BTreeSet<usize>,
}

#[derive(Default)]
struct TileIdToReferences {
    refs: BTreeMap<(u32, u32), TileReferences>,
}

impl TileIdToReferences {
    fn tile_ref_by_node(&mut self, node: &RawNode) -> &mut TileReferences {
        let node_tile = tile::coords_to_max_zoom_tile(node);
        self.refs
            .entry((node_tile.x, node_tile.y))
            .or_insert_with(Default::default)
    }

    fn tile_ref_by_xy(&mut self, tile_x: u32, tile_y: u32) -> &mut TileReferences {
        self.refs
            .entry((tile_x, tile_y))
            .or_insert_with(Default::default)
    }
}

fn save_to_internal_format<W>(writer: W, osm_xml: &ParsedOsmXml) -> Result<()>
where
    W: Write,
{
    let mut all_strings = BTreeSet::new();

    let mut nodes = Vec::new();
    for n in osm_xml.node_storage.entities.iter() {
        nodes.push(RawNode {
            global_id: n.global_id,
            lat: parse_required_attr(&n.initial_elem, "lat")?,
            lon: parse_required_attr(&n.initial_elem, "lon")?,
            tags: collect_tags(&n, &mut all_strings),
        });
    }

    let ways = osm_xml
        .way_storage
        .entities
        .iter()
        .map(|w| RawWay {
            global_id: w.global_id,
            node_ids: collect_references(w.get_elems_by_name("nd"), &osm_xml.node_storage),
            tags: collect_tags(&w, &mut all_strings),
        })
        .collect::<Vec<_>>();

    let relations = osm_xml
        .relation_storage
        .entities
        .iter()
        .map(|r| {
            let members = r.get_elems_by_name("member")
                .filter(|x| x.get_attr("type") == Some("way"));
            RawRelation {
                global_id: r.global_id,
                way_ids: collect_references(members, &osm_xml.way_storage),
                tags: collect_tags(&r, &mut all_strings),
            }
        })
        .collect::<Vec<_>>();

    let tile_references = get_tile_references(&nodes, &ways, &relations);

    Ok(())
}

fn get_tile_references(
    nodes: &[RawNode],
    ways: &[RawWay],
    relations: &[RawRelation],
) -> TileIdToReferences {
    let mut result: TileIdToReferences = Default::default();

    for (i, node) in nodes.iter().enumerate() {
        result.tile_ref_by_node(node).local_node_ids.insert(i);
    }

    for (i, way) in ways.iter().enumerate() {
        let node_ids = way.node_ids.iter().map(|idx| &nodes[*idx]);

        insert_entity_id_to_tiles(&mut result, node_ids, &|x| &mut x.local_way_ids, i);
    }

    for (i, relation) in relations.iter().enumerate() {
        let node_ids = relation
            .way_ids
            .iter()
            .flat_map(|way_id| ways[*way_id].node_ids.iter().map(|idx| &nodes[*idx]));

        insert_entity_id_to_tiles(&mut result, node_ids, &|x| &mut x.local_relation_ids, i);
    }

    result
}

fn insert_entity_id_to_tiles<'a, I>(
    result: &mut TileIdToReferences,
    mut nodes: I,
    get_refs: &Fn(&mut TileReferences) -> &mut BTreeSet<usize>,
    entity_id: usize,
) where
    I: Iterator<Item = &'a RawNode>,
{
    let first_node = match nodes.next() {
        Some(n) => n,
        _ => return,
    };

    let first_tile = tile::coords_to_max_zoom_tile(first_node);
    let mut tile_range = tile::TileRange {
        min_x: first_tile.x,
        max_x: first_tile.x,
        min_y: first_tile.y,
        max_y: first_tile.y,
    };
    for node in nodes {
        let next_tile = tile::coords_to_max_zoom_tile(node);
        tile_range.min_x = min(tile_range.min_x, next_tile.x);
        tile_range.max_x = max(tile_range.max_x, next_tile.x);
        tile_range.min_y = min(tile_range.min_y, next_tile.y);
        tile_range.max_y = max(tile_range.max_y, next_tile.y);
    }
    for x in tile_range.min_x..tile_range.max_x + 1 {
        for y in tile_range.min_y..tile_range.max_y + 1 {
            get_refs(result.tile_ref_by_xy(x, y)).insert(entity_id);
        }
    }
}
