use errors::*;

use coords;
use geodata::find_polygons::{find_polygons_in_multipolygon, NodeDesc, NodeDescPair};
use geodata::saver::save_to_internal_format;
use std::collections::HashSet;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use xml::attribute::OwnedAttribute;
use xml::reader::{EventReader, XmlEvent};

pub fn import(input: &str, output: &str) -> Result<()> {
    let input_file = File::open(input).chain_err(|| format!("Failed to open {} for reading", input))?;
    let output_file = File::create(output).chain_err(|| format!("Failed to open {} for writing", output))?;

    let parser = EventReader::new(BufReader::new(input_file));
    let mut writer = BufWriter::new(output_file);

    println!("Parsing XML");
    let parsed_xml = parse_osm_xml(parser)?;

    println!("Converting geodata to internal format");
    save_to_internal_format(&mut writer, &parsed_xml)
        .chain_err(|| "Failed to write the imported data to the output file")?;
    Ok(())
}

pub(super) struct OsmEntityStorage<E: Default> {
    global_id_to_local_id: HashMap<u64, usize>,
    entities: Vec<E>,
}

impl<E: Default> OsmEntityStorage<E> {
    fn new() -> OsmEntityStorage<E> {
        OsmEntityStorage {
            global_id_to_local_id: HashMap::new(),
            entities: Vec::new(),
        }
    }

    fn add(&mut self, global_id: u64, entity: E) {
        let old_size = self.entities.len();
        self.global_id_to_local_id.insert(global_id, old_size);
        self.entities.push(entity);
    }

    fn translate_id(&self, global_id: u64) -> Option<usize> {
        self.global_id_to_local_id.get(&global_id).cloned()
    }

    pub(super) fn get_entities(&self) -> &Vec<E> {
        &self.entities
    }
}

pub(super) struct EntityStorages {
    pub(super) node_storage: OsmEntityStorage<RawNode>,
    pub(super) way_storage: OsmEntityStorage<RawWay>,
    pub(super) polygon_storage: Vec<Polygon>,
    pub(super) multipolygon_storage: OsmEntityStorage<Multipolygon>,
}

fn parse_osm_xml<R: Read>(mut parser: EventReader<R>) -> Result<EntityStorages> {
    let mut entity_storages = EntityStorages {
        node_storage: OsmEntityStorage::new(),
        way_storage: OsmEntityStorage::new(),
        polygon_storage: Vec::new(),
        multipolygon_storage: OsmEntityStorage::new(),
    };

    let mut elem_count = 0;

    fn dump_state(entity_storages: &EntityStorages) -> String {
        format!(
            "{} nodes, {} ways and {} multipolygon relations",
            entity_storages.node_storage.entities.len(),
            entity_storages.way_storage.entities.len(),
            entity_storages.multipolygon_storage.entities.len()
        )
    }

    loop {
        let e = parser.next().chain_err(|| "Failed to parse the input file")?;
        match e {
            XmlEvent::EndDocument => break,
            XmlEvent::StartElement { name, attributes, .. } => {
                process_element(&name.local_name, &attributes, &mut entity_storages, &mut parser)?;
                elem_count += 1;
                if elem_count % 100_000 == 0 {
                    println!("Got {} so far", dump_state(&entity_storages));
                }
            }
            _ => {}
        }
    }

    println!("Total: {}", dump_state(&entity_storages));

    Ok(entity_storages)
}

fn process_element<R: Read>(
    name: &str,
    attrs: &[OwnedAttribute],
    entity_storages: &mut EntityStorages,
    parser: &mut EventReader<R>,
) -> Result<()> {
    match name {
        "node" => {
            let mut node = RawNode {
                global_id: get_id(name, attrs)?,
                lat: parse_required_attr(name, attrs, "lat")?,
                lon: parse_required_attr(name, attrs, "lon")?,
                tags: RawTags::default(),
            };
            process_subelements(name, &mut node, entity_storages, process_node_subelement, parser)?;
            entity_storages.node_storage.add(node.global_id, node);
        }
        "way" => {
            let mut way = RawWay {
                global_id: get_id(name, attrs)?,
                node_ids: RawRefs::default(),
                tags: RawTags::default(),
            };
            process_subelements(name, &mut way, entity_storages, process_way_subelement, parser)?;
            postprocess_node_refs(&mut way.node_ids);
            entity_storages.way_storage.add(way.global_id, way);
        }
        "relation" => {
            let mut relation = RawRelation {
                global_id: get_id(name, attrs)?,
                way_refs: Vec::<RelationWayRef>::default(),
                tags: RawTags::default(),
            };
            process_subelements(
                name,
                &mut relation,
                entity_storages,
                process_relation_subelement,
                parser,
            )?;
            if relation.tags.iter().any(|(k, v)| k == "type" && v == "multipolygon") {
                let segments = relation.to_segments(&entity_storages);
                if let Some(polygons) = find_polygons_in_multipolygon(relation.global_id, &segments) {
                    let mut multipolygon = Multipolygon {
                        global_id: relation.global_id,
                        polygon_ids: Vec::new(),
                        tags: relation.tags,
                    };
                    for poly in polygons {
                        multipolygon.polygon_ids.push(entity_storages.polygon_storage.len());
                        entity_storages.polygon_storage.push(poly);
                    }
                    entity_storages
                        .multipolygon_storage
                        .add(relation.global_id, multipolygon);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn process_subelements<E: Default, R: Read, F>(
    entity_name: &str,
    entity: &mut E,
    entity_storages: &EntityStorages,
    subelement_processor: F,
    parser: &mut EventReader<R>,
) -> Result<()>
where
    F: Fn(&mut E, &EntityStorages, &str, &[OwnedAttribute]) -> Result<()>,
{
    loop {
        let e = parser
            .next()
            .chain_err(|| format!("Failed to parse the input file when processing {}", entity_name))?;
        match e {
            XmlEvent::EndDocument => break,
            XmlEvent::EndElement { ref name } if name.local_name == *entity_name => break,
            XmlEvent::StartElement { name, attributes, .. } => {
                subelement_processor(entity, entity_storages, &name.local_name, &attributes)?
            }
            _ => {}
        }
    }
    Ok(())
}

fn postprocess_node_refs(refs: &mut RawRefs) {
    if refs.is_empty() {
        return;
    }

    let mut seen_node_pairs = HashSet::<(usize, usize)>::default();
    let mut refs_without_duplicates = vec![refs[0]];

    for idx in 1..refs.len() {
        let cur = refs[idx];
        let prev = refs[idx - 1];
        let node_pair = (cur, prev);
        if !seen_node_pairs.contains(&node_pair) && !seen_node_pairs.contains(&(prev, cur)) {
            seen_node_pairs.insert(node_pair);
            refs_without_duplicates.push(cur);
        }
    }

    *refs = refs_without_duplicates;
}

fn process_node_subelement(
    node: &mut RawNode,
    _: &EntityStorages,
    sub_name: &str,
    sub_attrs: &[OwnedAttribute],
) -> Result<()> {
    try_add_tag(sub_name, sub_attrs, &mut node.tags).map(|_| ())
}

fn process_way_subelement(
    way: &mut RawWay,
    entity_storages: &EntityStorages,
    sub_name: &str,
    sub_attrs: &[OwnedAttribute],
) -> Result<()> {
    if try_add_tag(sub_name, sub_attrs, &mut way.tags)? {
        return Ok(());
    }
    if sub_name == "nd" {
        if let Some(r) = get_ref(sub_name, sub_attrs, &entity_storages.node_storage)? {
            way.node_ids.push(r);
        }
    }
    Ok(())
}

fn process_relation_subelement(
    relation: &mut RawRelation,
    entity_storages: &EntityStorages,
    sub_name: &str,
    sub_attrs: &[OwnedAttribute],
) -> Result<()> {
    if try_add_tag(sub_name, sub_attrs, &mut relation.tags)? {
        return Ok(());
    }
    if sub_name == "member" && get_required_attr(sub_name, sub_attrs, "type")? == "way" {
        if let Some(r) = get_ref(sub_name, sub_attrs, &entity_storages.way_storage)? {
            let is_inner = get_required_attr(sub_name, sub_attrs, "role")? == "inner";
            relation.way_refs.push(RelationWayRef { way_id: r, is_inner });
        }
    }
    Ok(())
}

fn get_required_attr<'a>(elem_name: &str, attrs: &'a [OwnedAttribute], attr_name: &str) -> Result<&'a String> {
    attrs
        .iter()
        .filter(|x| x.name.local_name == attr_name)
        .map(|x| &x.value)
        .next()
        .ok_or_else(|| format!("Element {} doesn't have required attribute: {}", elem_name, attr_name).into())
}

fn parse_required_attr<T>(elem_name: &str, attrs: &[OwnedAttribute], attr_name: &str) -> Result<T>
where
    T: ::std::str::FromStr,
    T::Err: ::std::error::Error + ::std::marker::Send + 'static,
{
    let value = get_required_attr(elem_name, attrs, attr_name)?;

    let parsed_value = value.parse::<T>().chain_err(|| {
        format!(
            "Failed to parse the value of attribute {} ({}) for element {}",
            attr_name, value, elem_name
        )
    })?;

    Ok(parsed_value)
}

fn get_ref<E: Default>(
    elem_name: &str,
    attrs: &[OwnedAttribute],
    storage: &OsmEntityStorage<E>,
) -> Result<Option<usize>> {
    let reference = parse_required_attr(elem_name, attrs, "ref")?;
    Ok(storage.translate_id(reference))
}

fn try_add_tag<'a>(elem_name: &str, attrs: &'a [OwnedAttribute], tags: &mut RawTags) -> Result<bool> {
    if elem_name != "tag" {
        return Ok(false);
    }
    let key = get_required_attr(elem_name, attrs, "k")?;
    let value = get_required_attr(elem_name, attrs, "v")?;
    tags.insert(key.clone(), value.clone());
    Ok(true)
}

fn get_id(elem_name: &str, attrs: &[OwnedAttribute]) -> Result<u64> {
    parse_required_attr(elem_name, attrs, "id")
}

pub(super) type RawRefs = Vec<usize>;
pub(super) type RawTags = BTreeMap<String, String>;

#[derive(Default)]
pub(super) struct RawNode {
    pub(super) global_id: u64,
    pub(super) lat: f64,
    pub(super) lon: f64,
    pub(super) tags: RawTags,
}

impl coords::Coords for RawNode {
    fn lat(&self) -> f64 {
        self.lat
    }

    fn lon(&self) -> f64 {
        self.lon
    }
}

#[derive(Default)]
pub(super) struct RawWay {
    pub(super) global_id: u64,
    pub(super) node_ids: RawRefs,
    pub(super) tags: RawTags,
}

pub struct RelationWayRef {
    way_id: usize,
    is_inner: bool,
}

#[derive(Default)]
struct RawRelation {
    global_id: u64,
    way_refs: Vec<RelationWayRef>,
    tags: RawTags,
}

impl RawRelation {
    fn to_segments(&self, entity_storages: &EntityStorages) -> Vec<NodeDescPair> {
        let create_node_desc = |way: &RawWay, node_idx_in_way| {
            let node_id = way.node_ids[node_idx_in_way];
            let node = &entity_storages.node_storage.entities[node_id];
            NodeDesc::new(node_id, node.lat, node.lon)
        };
        self.way_refs
            .iter()
            .flat_map(|way_ref| {
                let way = &entity_storages.way_storage.entities[way_ref.way_id];
                (1..way.node_ids.len()).map(move |idx| {
                    NodeDescPair::new(
                        create_node_desc(way, idx - 1),
                        create_node_desc(way, idx),
                        way_ref.is_inner,
                    )
                })
            }).collect()
    }
}

pub(super) type Polygon = RawRefs;

#[derive(Default)]
pub(super) struct Multipolygon {
    pub(super) global_id: u64,
    pub(super) polygon_ids: RawRefs,
    pub(super) tags: RawTags,
}
