use crate::geodata::importer::{EntityStorages, Multipolygon, Polygon, RawNode, RawRefs, RawWay};
use crate::tile;
use anyhow::{bail, Result};
use byteorder::{LittleEndian, WriteBytesExt};
use std::cmp::{max, min};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;

#[derive(Default)]
struct TileReferences {
    local_node_ids: BTreeSet<usize>,
    local_way_ids: BTreeSet<usize>,
    local_multipolygon_ids: BTreeSet<usize>,
}

#[derive(Default)]
struct TileIdToReferences {
    refs: BTreeMap<(u32, u32), TileReferences>,
}

pub(super) fn save_to_internal_format(writer: &mut dyn Write, entity_storages: &EntityStorages) -> Result<()> {
    let mut buffered_data = BufferedData::default();
    let nodes = &entity_storages.node_storage.get_entities();
    save_nodes(writer, nodes, &mut buffered_data)?;

    let ways = &entity_storages.way_storage.get_entities();
    save_ways(writer, &ways, &mut buffered_data)?;

    let polygons = &entity_storages.polygon_storage;
    save_polygons(writer, &polygons, &mut buffered_data)?;

    let multipolygons = &entity_storages.multipolygon_storage.get_entities();
    save_multipolygons(writer, &multipolygons, &mut buffered_data)?;

    let tile_references = get_tile_references(&entity_storages);
    save_tile_references(writer, &tile_references, &mut buffered_data)?;

    buffered_data.save(writer)?;

    Ok(())
}

impl TileIdToReferences {
    fn tile_ref_by_node(&mut self, node: &RawNode) -> &mut TileReferences {
        let node_tile = tile::coords_to_max_zoom_tile(node);
        self.tile_ref_by_xy(node_tile.x, node_tile.y)
    }

    fn tile_ref_by_xy(&mut self, tile_x: u32, tile_y: u32) -> &mut TileReferences {
        self.refs.entry((tile_x, tile_y)).or_insert_with(Default::default)
    }
}

fn save_nodes(writer: &mut dyn Write, nodes: &[RawNode], data: &mut BufferedData) -> Result<()> {
    writer.write_u32::<LittleEndian>(to_u32_safe(nodes.len())?)?;
    for node in nodes {
        writer.write_u64::<LittleEndian>(node.global_id)?;
        writer.write_f64::<LittleEndian>(node.lat)?;
        writer.write_f64::<LittleEndian>(node.lon)?;
        save_tags(writer, &node.tags, data)?;
    }
    Ok(())
}

fn save_ways(writer: &mut dyn Write, ways: &[RawWay], data: &mut BufferedData) -> Result<()> {
    writer.write_u32::<LittleEndian>(to_u32_safe(ways.len())?)?;
    for way in ways {
        writer.write_u64::<LittleEndian>(way.global_id)?;
        save_refs(writer, way.node_ids.iter(), data)?;
        save_tags(writer, &way.tags, data)?;
    }
    Ok(())
}

fn save_polygons(writer: &mut dyn Write, polygons: &[Polygon], data: &mut BufferedData) -> Result<()> {
    writer.write_u32::<LittleEndian>(to_u32_safe(polygons.len())?)?;
    for polygon in polygons {
        save_refs(writer, polygon.iter(), data)?;
    }
    Ok(())
}

fn save_multipolygons(writer: &mut dyn Write, multipolygons: &[Multipolygon], data: &mut BufferedData) -> Result<()> {
    writer.write_u32::<LittleEndian>(to_u32_safe(multipolygons.len())?)?;
    for multipolygon in multipolygons {
        writer.write_u64::<LittleEndian>(multipolygon.global_id)?;
        save_refs(writer, multipolygon.polygon_ids.iter(), data)?;
        save_tags(writer, &multipolygon.tags, data)?;
    }
    Ok(())
}

fn save_tile_references(
    writer: &mut dyn Write,
    tile_references: &TileIdToReferences,
    data: &mut BufferedData,
) -> Result<()> {
    writer.write_u32::<LittleEndian>(to_u32_safe(tile_references.refs.len())?)?;
    for (k, v) in &tile_references.refs {
        writer.write_u32::<LittleEndian>(k.0)?;
        writer.write_u32::<LittleEndian>(k.1)?;

        save_refs(writer, v.local_node_ids.iter(), data)?;
        save_refs(writer, v.local_way_ids.iter(), data)?;
        save_refs(writer, v.local_multipolygon_ids.iter(), data)?;
    }

    Ok(())
}

fn save_refs<'a, I>(writer: &mut dyn Write, refs: I, data: &mut BufferedData) -> Result<()>
where
    I: Iterator<Item = &'a usize>,
{
    let offset = data.all_ints.len();
    for r in refs {
        data.all_ints.push(to_u32_safe(*r)?);
    }
    writer.write_u32::<LittleEndian>(to_u32_safe(offset)?)?;
    writer.write_u32::<LittleEndian>(to_u32_safe(data.all_ints.len() - offset)?)?;
    Ok(())
}

fn save_tags(writer: &mut dyn Write, tags: &BTreeMap<String, String>, data: &mut BufferedData) -> Result<()> {
    let mut kv_refs = RawRefs::new();

    for (ref k, ref v) in tags.iter() {
        let (k_offset, k_length) = data.add_string(k);
        let (v_offset, v_length) = data.add_string(v);
        kv_refs.extend([k_offset, k_length, v_offset, v_length].iter());
    }

    save_refs(writer, kv_refs.iter(), data)?;

    Ok(())
}

#[derive(Default)]
struct BufferedData {
    all_ints: Vec<u32>,
    string_to_offset: HashMap<String, usize>,
    all_strings: Vec<u8>,
}

impl BufferedData {
    fn add_string(&mut self, s: &str) -> (usize, usize) {
        let bytes = s.as_bytes();
        let all_strings = &mut self.all_strings;
        let offset = self.string_to_offset.entry(s.to_string()).or_insert_with(|| {
            let offset = all_strings.len();
            all_strings.extend_from_slice(bytes);
            offset
        });
        (*offset, bytes.len())
    }

    fn save(&self, writer: &mut dyn Write) -> Result<()> {
        writer.write_u32::<LittleEndian>(to_u32_safe(self.all_ints.len())?)?;
        for i in &self.all_ints {
            writer.write_u32::<LittleEndian>(*i)?;
        }
        writer.write_all(&self.all_strings)?;
        Ok(())
    }
}

fn get_tile_references(entity_storages: &EntityStorages) -> TileIdToReferences {
    let mut result = TileIdToReferences::default();

    let nodes = &entity_storages.node_storage.get_entities();
    for (i, node) in nodes.iter().enumerate() {
        result.tile_ref_by_node(node).local_node_ids.insert(i);
    }

    for (i, way) in entity_storages.way_storage.get_entities().iter().enumerate() {
        let node_ids = way.node_ids.iter().map(|idx| &nodes[*idx]);

        insert_entity_id_to_tiles(&mut result, node_ids, |x| &mut x.local_way_ids, i);
    }

    let polygons = &entity_storages.polygon_storage;
    for (i, multipolygon) in entity_storages.multipolygon_storage.get_entities().iter().enumerate() {
        let node_ids = multipolygon
            .polygon_ids
            .iter()
            .flat_map(move |poly_id| polygons[*poly_id].iter())
            .map(|idx| &nodes[*idx]);
        insert_entity_id_to_tiles(&mut result, node_ids, |x| &mut x.local_multipolygon_ids, i);
    }

    result
}

fn insert_entity_id_to_tiles<'a, I>(
    result: &mut TileIdToReferences,
    mut nodes: I,
    get_refs: impl Fn(&mut TileReferences) -> &mut BTreeSet<usize>,
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
    for x in tile_range.min_x..=tile_range.max_x {
        for y in tile_range.min_y..=tile_range.max_y {
            get_refs(result.tile_ref_by_xy(x, y)).insert(entity_id);
        }
    }
}

fn to_u32_safe(num: usize) -> Result<u32> {
    if num > (u32::max_value() as usize) {
        bail!("{} doesn't fit into u32", num);
    }
    Ok(num as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::BufWriter;

    #[test]
    fn test_synthetic_data() {
        let mut good_node_ids = Vec::new();
        let mut tile_ids = Vec::new();

        {
            let mut add_tile = |x, y, good| {
                let node_idx = tile_ids.len();
                tile_ids.push((x, y));
                if good {
                    good_node_ids.push(node_idx as u32);
                }
            };

            // y = {8, 9, 13} are in the range for x = 1
            add_tile(1, 7, false);
            add_tile(1, 8, true);
            add_tile(1, 9, true);
            add_tile(1, 13, true);
            // y = {10, 11, 15} is in the range for x = 2
            add_tile(2, 10, true);
            add_tile(2, 11, true);
            add_tile(2, 15, true);
            add_tile(2, 16, false);
            add_tile(2, 17, false);
            // nothing is in the range for x = 4
            add_tile(4, 1, false);
            add_tile(4, 4, false);
            // nothing is in the range for x = 5
            add_tile(5, 20, false);
            add_tile(5, 23, false);
            add_tile(5, 200, false);
            // y = {11, 12, 14} are in the range for x = 7
            add_tile(7, 6, false);
            add_tile(7, 11, true);
            add_tile(7, 12, true);
            add_tile(7, 14, true);
            add_tile(7, 16, false);
            add_tile(7, 17, false);
        }

        let mut nodes = Vec::new();
        for idx in 0..tile_ids.len() {
            nodes.push(RawNode {
                global_id: idx as u64,
                lat: 1.0,
                lon: 1.0,
                tags: crate::geodata::importer::RawTags::default(),
            });
        }

        let mut tile_refs = TileIdToReferences::default();
        for (idx, &(x, y)) in tile_ids.iter().enumerate() {
            tile_refs.refs.entry((x, y)).or_insert(TileReferences {
                local_node_ids: [idx].iter().cloned().collect(),
                local_way_ids: BTreeSet::default(),
                local_multipolygon_ids: BTreeSet::default(),
            });
        }

        let mut tmp_path = env::temp_dir();
        tmp_path.push("osm_renderer_synthetic_test.bin");

        {
            let tmp_file = File::create(&tmp_path).unwrap();
            let mut writer = BufWriter::new(tmp_file);

            let mut data = BufferedData::default();
            save_nodes(&mut writer, &nodes, &mut data).unwrap();
            save_ways(&mut writer, &[], &mut data).unwrap();
            save_polygons(&mut writer, &[], &mut data).unwrap();
            save_multipolygons(&mut writer, &[], &mut data).unwrap();
            save_tile_references(&mut writer, &tile_refs, &mut data).unwrap();
            data.save(&mut writer).unwrap();
        }

        let reader = crate::geodata::reader::GeodataReader::load(tmp_path.to_str().unwrap()).unwrap();
        let tile = crate::tile::Tile { zoom: 15, x: 0, y: 1 };
        let mut local_ids = crate::geodata::reader::OsmEntityIds::default();
        reader.get_entities_in_tile(&tile, &mut local_ids);
        assert_eq!(good_node_ids, local_ids.nodes);
    }
}
