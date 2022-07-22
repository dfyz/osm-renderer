use crate::geodata::importer::{EntityStorages, Multipolygon, Polygon, RawNode, RawRefs, RawWay};
use crate::tile;
use anyhow::{bail, Result};
use byteorder::{LittleEndian, WriteBytesExt};
use std::cmp::{max, min};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;

const LOCAL_NODE: u8 = 0;
const LOCAL_WAY: u8 = 1;
const LOCAL_MULTIPOLYGON: u8 = 2;
const LOCAL_COUNT: usize = 3;

type TileIdToReferences = BTreeSet<(u32, u32, u8, u32)>;

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

    let tile_references = get_tile_references(&entity_storages)?;
    save_tile_references(writer, &tile_references, &mut buffered_data)?;

    buffered_data.save(writer)?;

    Ok(())
}

fn save_nodes(writer: &mut dyn Write, nodes: &[RawNode], data: &mut BufferedData) -> Result<()> {
    writer.write_u32::<LittleEndian>(nodes.len().to_u32_safe()?)?;
    for node in nodes {
        writer.write_u64::<LittleEndian>(node.global_id)?;
        writer.write_f64::<LittleEndian>(node.lat)?;
        writer.write_f64::<LittleEndian>(node.lon)?;
        save_tags(writer, &node.tags, data)?;
    }
    Ok(())
}

fn save_ways(writer: &mut dyn Write, ways: &[RawWay], data: &mut BufferedData) -> Result<()> {
    writer.write_u32::<LittleEndian>(ways.len().to_u32_safe()?)?;
    for way in ways {
        writer.write_u64::<LittleEndian>(way.global_id)?;
        save_refs(writer, way.node_ids.iter(), data)?;
        save_tags(writer, &way.tags, data)?;
    }
    Ok(())
}

fn save_polygons(writer: &mut dyn Write, polygons: &[Polygon], data: &mut BufferedData) -> Result<()> {
    writer.write_u32::<LittleEndian>(polygons.len().to_u32_safe()?)?;
    for polygon in polygons {
        save_refs(writer, polygon.iter(), data)?;
    }
    Ok(())
}

fn save_multipolygons(writer: &mut dyn Write, multipolygons: &[Multipolygon], data: &mut BufferedData) -> Result<()> {
    writer.write_u32::<LittleEndian>(multipolygons.len().to_u32_safe()?)?;
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
    let mut unique_tile_count: usize = 0;
    let mut cur_tile_id: Option<(u32, u32)> = None;
    for (x, y, _, _) in tile_references {
        if cur_tile_id.is_none() || cur_tile_id.unwrap() != (*x, *y) {
            unique_tile_count += 1;
        }
        cur_tile_id = Some((*x, *y));
    }
    writer.write_u32::<LittleEndian>(unique_tile_count.to_u32_safe()?)?;

    cur_tile_id = None;
    let mut cur_offset: usize = data.all_ints.len();
    let mut counts: [usize; LOCAL_COUNT] = [0; LOCAL_COUNT];

    let mut dump_counts = |w: &mut dyn Write, counts: [usize; LOCAL_COUNT]| -> Result<()> {
        for cnt in counts {
            // println!("{} {}", cur_offset, cnt);
            w.write_u32::<LittleEndian>(cur_offset.to_u32_safe()?)?;
            w.write_u32::<LittleEndian>(cnt.to_u32_safe()?)?;
            cur_offset += cnt;
        }
        Ok(())
    };

    for (x, y, entity_type, entity_ref) in tile_references.iter() {
        if cur_tile_id.is_none() || cur_tile_id.unwrap() != (*x, *y) {
            if cur_tile_id.is_some() {
                dump_counts(writer, counts)?;
                counts = [0; LOCAL_COUNT];
            }
            // println!("new tile: {:?}", (*x, *y));
            writer.write_u32::<LittleEndian>(*x)?;
            writer.write_u32::<LittleEndian>(*y)?;
            cur_tile_id = Some((*x, *y));
        }

        data.all_ints.push(*entity_ref);
        counts[*entity_type as usize] += 1;
    }

    if cur_tile_id.is_some() {
        dump_counts(writer, counts)?;
    }

    Ok(())
}

fn save_refs<'a, I, N>(writer: &mut dyn Write, refs: I, data: &mut BufferedData) -> Result<()>
where
    N: ConvertableToU32 + Copy + 'static,
    I: Iterator<Item = &'a N>,
{
    let offset = data.all_ints.len();
    for r in refs {
        data.all_ints.push(r.to_u32_safe()?);
    }
    writer.write_u32::<LittleEndian>(offset.to_u32_safe()?)?;
    writer.write_u32::<LittleEndian>((data.all_ints.len() - offset).to_u32_safe()?)?;
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
        writer.write_u32::<LittleEndian>(self.all_ints.len().to_u32_safe()?)?;
        for i in &self.all_ints {
            writer.write_u32::<LittleEndian>(*i)?;
        }
        writer.write_all(&self.all_strings)?;
        Ok(())
    }
}

fn get_tile_references(entity_storages: &EntityStorages) -> Result<TileIdToReferences> {
    let mut result = TileIdToReferences::default();

    let nodes = &entity_storages.node_storage.get_entities();
    for (i, node) in nodes.iter().enumerate() {
        let node_tile = tile::coords_to_max_zoom_tile(node);
        result.insert((node_tile.x, node_tile.y, LOCAL_NODE, i.to_u32_safe()?));
    }

    for (i, way) in entity_storages.way_storage.get_entities().iter().enumerate() {
        let node_ids = way.node_ids.iter().map(|idx| &nodes[*idx]);

        insert_entity_id_to_tiles(&mut result, node_ids, LOCAL_WAY, i)?;
    }

    let polygons = &entity_storages.polygon_storage;
    for (i, multipolygon) in entity_storages.multipolygon_storage.get_entities().iter().enumerate() {
        let node_ids = multipolygon
            .polygon_ids
            .iter()
            .flat_map(move |poly_id| polygons[*poly_id].iter())
            .map(|idx| &nodes[*idx]);
        insert_entity_id_to_tiles(&mut result, node_ids, LOCAL_MULTIPOLYGON, i)?;
    }

    Ok(result)
}

fn insert_entity_id_to_tiles<'a, I>(
    result: &mut TileIdToReferences,
    mut nodes: I,
    entity_type: u8,
    entity_id: usize,
) -> Result<()> where
    I: Iterator<Item = &'a RawNode>,
{
    let first_node = match nodes.next() {
        Some(n) => n,
        _ => return Ok(()),
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
            result.insert((x, y, entity_type, entity_id.to_u32_safe()?));
        }
    }

    Ok(())
}

trait ConvertableToU32 {
    fn to_u32_safe(self) -> Result<u32>;
}

impl ConvertableToU32 for usize {
    fn to_u32_safe(self) -> Result<u32> {
        if self > (u32::max_value() as usize) {
            bail!("{} doesn't fit into u32", self);
        }
        Ok(self as u32)
    }
}

impl ConvertableToU32 for u32 {
    fn to_u32_safe(self) -> Result<u32> {
        Ok(self)
    }
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
            tile_refs.insert((x, y, LOCAL_NODE, idx as u32));
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
