use errors::*;

use byteorder::{LittleEndian, WriteBytesExt};
use geodata::importer::{
    save_refs, save_tags, to_u32_safe, BufferedData, EntityStorages, RawRefs, RawRelation, RawTags,
};
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

pub(super) fn convert_relation_to_multipolygon(entity_storages: &mut EntityStorages, relation: &RawRelation) {}

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
