use errors::*;

use coords;
use tile;

use std::cmp::{max, min};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read};

use capnp::message::{Allocator, Builder};
use geodata_capnp::{geodata, node, tag_list};
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

    info!("Parsing XML");
    let parsed_xml = parse_osm_xml(parser)?;

    info!("Converting geodata to internal format");
    let mut message = Builder::new_default();
    convert_to_message(&mut message, &parsed_xml)?;

    ::capnp::serialize::write_message(&mut writer, &message)
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

    fn get_attr(&self, name: &str) -> Option<&String> {
        self.attrs
            .binary_search_by(|probe| {
                let probe_str: &str = probe.0.as_ref();
                probe_str.cmp(name)
            })
            .ok()
            .map(|idx| &self.attrs[idx].1)
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

    fn get_elems_by_name<'a>(&'a self, name: &str) -> Vec<&'a OsmXmlElement> {
        self.additional_elems
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
        self.global_id_to_local_id
            .insert(entity.global_id, old_size);
        self.entities.push(entity);
    }

    fn translate_id(&self, global_id: u64) -> Option<usize> {
        let result = self.global_id_to_local_id.get(&global_id);
        if result.is_none() {
            warn!("Failed to find an entity with ID = {}", global_id);
        }
        result.cloned()
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
                    info!(
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
            let new_entity = OsmEntity::new(osm_elem);
            if new_entity.is_some() {
                parsing_state.current_entity_with_type = new_entity.map(|x| (x, entity_type));
            }
        }
    }
}

fn process_end_element(name: &OwnedName, parsing_state: &mut ParsedOsmXml) {
    let is_final_entity_element =
        if let Some((_, ref entity_type)) = parsing_state.current_entity_with_type {
            *entity_type == name.local_name
        } else {
            false
        };

    if !is_final_entity_element {
        return;
    }

    let (entity, entity_type) = parsing_state.current_entity_with_type.take().unwrap();

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

fn get_required_attr(osm_elem: &OsmXmlElement, attr_name: &str) -> Result<String> {
    match osm_elem.get_attr(attr_name) {
        Some(value) => Ok(value.clone()),
        None => bail!(
            "Element {} doesn't have required attribute: {}",
            osm_elem,
            attr_name
        ),
    }
}

fn parse_required_attr<T>(osm_elem: &OsmXmlElement, attr_name: &str) -> Result<T>
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

fn collect_tags(tag_builder: &mut tag_list::Builder, osm_entity: &OsmEntity) -> Result<()> {
    let mut tags_in = osm_entity
        .get_elems_by_name("tag")
        .into_iter()
        .filter_map(
            |x| match (get_required_attr(x, "k"), get_required_attr(x, "v")) {
                (Ok(k), Ok(v)) => Some((k, v)),
                _ => None,
            },
        )
        .collect::<Vec<_>>();

    tags_in.sort();

    let mut tags_out = tag_builder.borrow().init_tags(tags_in.len() as u32);

    for (i, &(ref k, ref v)) in tags_in.iter().enumerate() {
        let mut tag_out = tags_out.borrow().get(i as u32);
        tag_out.set_key(k);
        tag_out.set_value(v);
    }

    Ok(())
}

macro_rules! fill_message_part {
    ($entity_in:ident, $entity_out:ident, $geodata:expr, $init_part:ident, $storage:expr, $local_id:ident, $fill_part:block) => {{
        let mut entities_out = $geodata.borrow().$init_part($storage.entities.len() as u32);
        for (i, $entity_in) in $storage.entities.iter().enumerate() {
            let $local_id = i as u32;
            let mut $entity_out = entities_out.borrow().get($local_id);
            $entity_out.set_global_id($entity_in.global_id);
            $fill_part
            collect_tags(&mut $entity_out.init_tags(), &$entity_in)?;
        }
    }}
}

macro_rules! collect_references {
    ($refs_in:expr, $entity:expr, $init_part:ident, $storage:expr) => {{
        let valid_refs = $refs_in
            .iter()
            .filter_map(|x| {
                x
                    .get_attr("ref")
                    .and_then(|y| y.parse().ok())
                    .and_then(|y| $storage.translate_id(y))
            })
            .collect::<Vec<_>>();
        let mut refs_out = $entity.borrow().$init_part(valid_refs.len() as u32);
        for (i, local_ref_id) in valid_refs.iter().enumerate() {
            refs_out.set(i as u32, *local_ref_id as u32);
        }
    }}
}

macro_rules! copy_vec_to_capnproto {
    ($entity:expr, $init_array:ident, $vec:expr) => {{
        let mut elems_out = $entity.borrow().$init_array($vec.len() as u32);
        for (i, elem_in) in $vec.iter().enumerate() {
            elems_out.set(i as u32, *elem_in);
        }
    }}
}

#[derive(Default)]
struct TileReferences {
    local_node_ids: BTreeSet<u32>,
    local_way_ids: BTreeSet<u32>,
    local_relation_ids: BTreeSet<u32>,
}

#[derive(Default)]
struct TileIdToReferences {
    refs: BTreeMap<(u32, u32), TileReferences>,
}

type NodeReader<'a> = ::capnp::struct_list::Reader<'a, node::Owned>;

impl TileIdToReferences {
    fn tile_ref_by_node<'a>(
        &mut self,
        nodes: NodeReader<'a>,
        local_node_id: u32,
    ) -> &mut TileReferences {
        let node_tile = tile::coords_to_max_zoom_tile(&get_coords_for_node(nodes, local_node_id));
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

fn get_coords_for_node(nodes: NodeReader, local_node_id: u32) -> ::geodata_capnp::coords::Reader {
    nodes.get(local_node_id).get_coords().unwrap()
}

impl<'a> coords::Coords for ::geodata_capnp::coords::Reader<'a> {
    fn lat(&self) -> f64 {
        self.get_lat()
    }

    fn lon(&self) -> f64 {
        self.get_lon()
    }
}

fn convert_to_message<A: Allocator>(
    message: &mut Builder<A>,
    osm_xml: &ParsedOsmXml,
) -> Result<()> {
    let mut geodata = message.init_root::<geodata::Builder>();

    fill_message_part!(
        node_in,
        node_out,
        geodata,
        init_nodes,
        osm_xml.node_storage,
        local_id,
        {
            let mut coords = node_out.borrow().init_coords();

            coords.set_lat(parse_required_attr(&node_in.initial_elem, "lat")?);
            coords.set_lon(parse_required_attr(&node_in.initial_elem, "lon")?);
        }
    );

    fill_message_part!(
        way_in,
        way_out,
        geodata,
        init_ways,
        osm_xml.way_storage,
        local_id,
        {
            let nds_in = way_in.get_elems_by_name("nd");
            collect_references!(nds_in, way_out, init_local_node_ids, osm_xml.node_storage);
        }
    );

    fill_message_part!(
        rel_in,
        rel_out,
        geodata,
        init_relations,
        osm_xml.relation_storage,
        local_id,
        {
            let members = rel_in.get_elems_by_name("member");

            let collect_members = |member_type| {
                members
                    .iter()
                    .filter(|x| x.get_attr("type").map(|x| x.as_ref()) == Some(member_type))
                    .collect::<Vec<_>>()
            };

            collect_references!(
                collect_members("node"),
                rel_out,
                init_local_node_ids,
                osm_xml.node_storage
            );
            collect_references!(
                collect_members("way"),
                rel_out,
                init_local_way_ids,
                osm_xml.way_storage
            );
        }
    );

    let tile_references = get_tile_references(geodata.borrow_as_reader());

    let mut tiles = geodata.init_tiles(tile_references.refs.len() as u32);
    for (i, (&(tx, ty), value)) in tile_references.refs.iter().enumerate() {
        let mut tile = tiles.borrow().get(i as u32);

        tile.set_tile_x(tx);
        tile.set_tile_y(ty);

        copy_vec_to_capnproto!(tile, init_local_node_ids, value.local_node_ids);
        copy_vec_to_capnproto!(tile, init_local_way_ids, value.local_way_ids);
        copy_vec_to_capnproto!(tile, init_local_relation_ids, value.local_relation_ids);
    }

    Ok(())
}

fn get_tile_references(geodata: geodata::Reader) -> TileIdToReferences {
    fn insert_entity_id_to_tiles<'a>(
        result: &mut TileIdToReferences,
        nodes: NodeReader<'a>,
        nodes_are_isolated: bool,
        local_node_ids: ::capnp::primitive_list::Reader<'a, u32>,
        get_refs: &Fn(&mut TileReferences) -> &mut BTreeSet<u32>,
        entity_id: u32,
    ) {
        if local_node_ids.len() == 0 {
            return;
        }
        if nodes_are_isolated || local_node_ids.len() == 1 {
            for node_ref in local_node_ids.iter() {
                get_refs(result.tile_ref_by_node(nodes, node_ref)).insert(entity_id);
            }
            return;
        }

        let first_tile =
            tile::coords_to_max_zoom_tile(&get_coords_for_node(nodes, local_node_ids.get(0)));
        let mut tile_range = tile::TileRange {
            min_x: first_tile.x,
            max_x: first_tile.x,
            min_y: first_tile.y,
            max_y: first_tile.y,
        };
        for i in 1..local_node_ids.len() {
            let next_tile =
                tile::coords_to_max_zoom_tile(&get_coords_for_node(nodes, local_node_ids.get(i)));
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

    let mut result: TileIdToReferences = Default::default();
    let all_nodes = geodata.get_nodes().unwrap();

    for i in 0..all_nodes.len() {
        result
            .tile_ref_by_node(all_nodes, i)
            .local_node_ids
            .insert(i);
    }

    let all_ways = geodata.get_ways().unwrap();
    for i in 0..all_ways.len() {
        let local_node_ids = all_ways.get(i).get_local_node_ids().unwrap();
        insert_entity_id_to_tiles(
            &mut result,
            all_nodes,
            false,
            local_node_ids,
            &|x| &mut x.local_way_ids,
            i,
        );
    }

    let all_relations = geodata.get_relations().unwrap();
    for i in 0..all_relations.len() {
        let local_node_ids = all_relations.get(i).get_local_node_ids().unwrap();
        insert_entity_id_to_tiles(
            &mut result,
            all_nodes,
            true,
            local_node_ids,
            &|x| &mut x.local_relation_ids,
            i,
        );

        let local_ways_ids = all_relations.get(i).get_local_way_ids().unwrap();
        for way_ref in local_ways_ids.iter() {
            let local_node_ids = all_ways.get(way_ref).get_local_node_ids().unwrap();
            insert_entity_id_to_tiles(
                &mut result,
                all_nodes,
                false,
                local_node_ids,
                &|x| &mut x.local_relation_ids,
                i,
            );
        }
    }

    result
}
