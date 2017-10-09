#!/usr/bin/env python

from __future__ import print_function

import collections
import json
import mercantile
import xml.etree.ElementTree as ET


INPUT_FILE = 'nano_moscow.osm'
OUTPUT_FILE = 'test_data.json'
BOUNDS = (37.608505, 55.750717, 37.619706, 55.756187)
ZOOMS = range(14, 18 + 1)


def main():
    root = ET.parse(INPUT_FILE).getroot()

    def insert_tags(elem, elem_id, tag_dict):
        for tag in elem.findall('tag'):
            tag_dict[elem_id].append((tag.attrib['k'], tag.attrib['v']))

    node_to_coords = {}
    node_to_tags = collections.defaultdict(list)
    for node in root.findall('node'):
        a = node.attrib
        node_id = a['id']
        node_to_coords[node_id] = (float(a['lat']), float(a['lon']))
        insert_tags(node, node_id, node_to_tags)

    way_to_nodes = {}
    way_to_tags = collections.defaultdict(list)
    for way in root.findall('way'):
        a = way.attrib
        way_id = a['id']
        way_to_nodes[way_id] = [w.attrib['ref'] for w in way.findall('nd')]
        insert_tags(way, way_id, way_to_tags)

    relation_to_ways = {}
    relation_to_nodes = {}
    relation_to_tags = collections.defaultdict(list)
    for rel in root.findall('relation'):
        a = rel.attrib
        rel_id = a['id']

        def filter_by_type(t):
            return set([m.attrib['ref'] for m in rel.findall('member') if m.attrib['type'] == t])

        relation_to_nodes[rel_id] = filter_by_type('node')
        relation_to_ways[rel_id] = filter_by_type('way')
        insert_tags(rel, rel_id, relation_to_tags)

    test_data = []
    for tile in mercantile.tiles(*BOUNDS, zooms=ZOOMS):
        bounds = mercantile.bounds(tile)

        def is_good_node(lat, lon):
            return (bounds.south < lat <= bounds.north) and (bounds.west <= lon < bounds.east)

        def is_good_way(refs):
            node_list = [node_to_coords[r] for r in refs if r in node_to_coords]
            lat, lon = node_list[0]
            south, north, west, east = lat, lat, lon, lon
            for i in range(1, len(node_list)):
                lat, lon = node_list[i]
                south = min(south, lat)
                north = max(north, lat)
                west = min(west, lon)
                east = max(east, lon)
            return tile in mercantile.tiles(west, south, east, north, [tile.z])

        good_nodes = set([node_id for node_id, coords in node_to_coords.items() if is_good_node(*coords)])
        good_ways = set([way_id for way_id, refs in way_to_nodes.items() if (set(refs) & good_nodes) or is_good_way(refs)])
        good_relations = set(
            [rel_id for rel_id, refs in relation_to_nodes.items() if refs & good_nodes] +
            [rel_id for rel_id, refs in relation_to_ways.items() if refs & good_ways]
        )

        def with_tags(entity_ids, entity_to_tags):
            result = {}
            for entity_id in entity_ids:
                result[entity_id] = dict(entity_to_tags[entity_id])
            return result

        tile_data = {
            'zoom': tile.z,
            'x': tile.x,
            'y': tile.y,
            'nodes': with_tags(good_nodes, node_to_tags),
            'ways': with_tags(good_ways, way_to_tags),
            'relations': with_tags(good_relations, relation_to_tags),
        }

        test_data.append(tile_data)

    with open(OUTPUT_FILE, 'w') as out:
        json.dump(test_data, out, sort_keys=True)


if __name__ == '__main__':
    main()
