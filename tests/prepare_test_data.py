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
        way_to_nodes[a['id']] = [w.attrib['ref'] for w in way.findall('nd')]

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
    for tile in mercantile.tiles(*BOUNDS, zooms=ZOOMS):
        bounds = mercantile.bounds(tile)

        def is_good_node(lat, lon):
            return (bounds.south < lat <= bounds.north) and (bounds.west <= lon < bounds.east)

        def is_good_way(refs):
            node_list = [node_to_coords[r] for r in refs if r in node_to_coords]
            for i in range(1, len(node_list)):
                (lat1, lon1) = node_list[i - 1]
                (lat2, lon2) = node_list[i]
                south = min(lat1, lat2)
                north = max(lat1, lat2)
                west = min(lon1, lon2)
                east = max(lon1, lon2)
                if tile in mercantile.tiles(west, south, east, north, [tile.z]):
                    return True
            return False

        good_nodes = set([node_id for node_id, coords in node_to_coords.items() if is_good_node(*coords)])
        good_ways = set([way_id for way_id, refs in way_to_nodes.items() if (set(refs) & good_nodes) or is_good_way(refs)])
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
