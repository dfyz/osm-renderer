#!/usr/bin/env python

from __future__ import print_function

import json
import mercantile
import xml.etree.ElementTree as ET


INPUT_FILE = 'nano_moscow.osm'
OUTPUT_FILE = 'test_data.json'
BOUNDS = (37.608505, 55.750717, 37.619706, 55.756187)
ZOOMS = range(14, 18 + 1)


def main():
    root = ET.parse(INPUT_FILE).getroot()

    node_to_coords = {}
    for node in root.findall('node'):
        a = node.attrib
        node_to_coords[a['id']] = (float(a['lat']), float(a['lon']))

    way_to_nodes = {}
    for way in root.findall('way'):
        a = way.attrib
        way_to_nodes[a['id']] = set([w.attrib['ref'] for w in way.findall('nd')])

    relation_to_ways = {}
    relation_to_nodes = {}
    for rel in root.findall('relation'):
        a = rel.attrib
        rel_id = a['id']

        def filter_by_type(t):
            return set([m.attrib['ref'] for m in rel.findall('member') if m.attrib['type'] == t])

        relation_to_nodes[rel_id] = filter_by_type('node')
        relation_to_ways[rel_id] = filter_by_type('way')

    test_data = []
    for tile in mercantile.tiles(*BOUNDS, ZOOMS):
        bounds = mercantile.bounds(tile)

        def is_good(lat, lon):
            return (bounds.south < lat <= bounds.north) and (bounds.west <= lon < bounds.east)

        good_nodes = set([node_id for node_id, coords in node_to_coords.items() if is_good(*coords)])
        good_ways = set([way_id for way_id, refs in way_to_nodes.items() if refs & good_nodes])
        good_relations = set(
            [rel_id for rel_id, refs in relation_to_nodes.items() if refs & good_nodes] +
            [rel_id for rel_id, refs in relation_to_ways.items() if refs & good_ways]
        )

        tile_data = {
            'zoom': tile.z,
            'x': tile.x,
            'y': tile.y,
            'nodes': list(good_nodes),
            'ways': list(good_ways),
            'relations': list(good_relations),
        }

        test_data.append(tile_data)

    with open(OUTPUT_FILE, 'w') as out:
        json.dump(test_data, out)


if __name__ == '__main__':
    main()
