use errors::*;

use byteorder::{LittleEndian, WriteBytesExt};
use geodata::importer::{save_refs, save_tags, to_u32_safe, BufferedData, EntityStorages, RawRefs, RawTags};
use std::collections::{HashSet, HashMap};
use std::io::Write;

#[derive(Default)]
pub(super) struct Polygon {
    node_ids: RawRefs,
}

#[derive(Default)]
pub(super) struct Multipolygon {
    global_id: u64,
    polygon_ids: RawRefs,
    tags: RawTags,
}

pub(super) struct NodeDesc {
    id: usize,
    pos: (u64, u64),
}

impl NodeDesc {
    pub(super) fn new(id: usize, lat: f64, lon: f64) -> NodeDesc {
        NodeDesc {
            id,
            pos: (lat.to_bits(), lon.to_bits()),
        }
    }
}

pub(super) fn convert_relation_to_multipolygon(
    entity_storages: &mut EntityStorages,
    relation_id: u64,
    relation_segments: &[Vec<NodeDesc>],
    relation_tags: RawTags,
) {
    let mut endpoint_to_segment: HashMap<(u64, u64), Vec<usize>> = HashMap::default();
    for (idx, seg) in relation_segments.iter().enumerate() {
        if !seg.is_empty() && (seg[0].pos != seg[seg.len() - 1].pos) {
            endpoint_to_segment.entry(seg[0].pos).or_default().push(idx);
            endpoint_to_segment.entry(seg[seg.len() - 1].pos).or_default().push(idx);
        }
    }

    for (_, v) in endpoint_to_segment.iter() {
        if v.len() % 2 != 0 {
            eprintln!("Relation {} is invalid", relation_id);
            break;
        }
    }
}

pub(super) fn save_polygons(writer: &mut Write, polygons: &[Polygon], data: &mut BufferedData) -> Result<()> {
    writer.write_u32::<LittleEndian>(to_u32_safe(polygons.len())?)?;
    for polygon in polygons {
        save_refs(writer, polygon.node_ids.iter(), data)?;
    }
    Ok(())
}

pub(super) fn save_multipolygons(
    writer: &mut Write,
    multipolygons: &[Multipolygon],
    data: &mut BufferedData,
) -> Result<()> {
    writer.write_u32::<LittleEndian>(to_u32_safe(multipolygons.len())?)?;
    for multipolygon in multipolygons {
        writer.write_u64::<LittleEndian>(multipolygon.global_id)?;
        save_refs(writer, multipolygon.polygon_ids.iter(), data)?;
        save_tags(writer, &multipolygon.tags, data)?;
    }
    Ok(())
}

pub(super) fn to_node_ids<'a>(
    multipolygon: &'a Multipolygon,
    polygons: &'a [Polygon],
) -> impl Iterator<Item = &'a usize> {
    multipolygon
        .polygon_ids
        .iter()
        .flat_map(move |poly_id| polygons[*poly_id].node_ids.iter())
}
