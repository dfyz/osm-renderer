use geodata::importer::Polygon;
use std::collections::{HashMap, HashSet};

type NodePos = (u64, u64);

pub(super) struct NodeDesc {
    id: usize,
    pos: NodePos,
}

impl NodeDesc {
    pub(super) fn new(id: usize, lat: f64, lon: f64) -> NodeDesc {
        NodeDesc {
            id,
            pos: (lat.to_bits(), lon.to_bits()),
        }
    }
}

pub(super) struct NodeDescPair {
    node1: NodeDesc,
    node2: NodeDesc,
    is_inner: bool,
}

impl NodeDescPair {
    pub(super) fn new(node1: NodeDesc, node2: NodeDesc, is_inner: bool) -> NodeDescPair {
        NodeDescPair { node1, node2, is_inner }
    }
}

pub(super) fn find_polygons_in_multipolygon(
    relation_id: u64,
    relation_segments: &[NodeDescPair],
) -> Option<Vec<Polygon>> {
    let connections = get_connections(relation_segments);
    let mut unmatched_count = relation_segments.len();
    let mut available_segments = vec![true; relation_segments.len()];
    let mut all_rings = Vec::new();

    while unmatched_count > 0 {
        match find_ring(relation_segments, &connections, &mut available_segments) {
            Some(ring) => {
                unmatched_count -= ring.len();
                all_rings.push(ring);
            }
            None => {
                eprintln!(
                    "Relation #{} is not a valid multipolygon (built {} complete rings, but {} segments are unmatched)",
                    relation_id,
                    all_rings.len(),
                    unmatched_count,
                );
                return None;
            }
        }
    }

    let mut polygons = Vec::new();
    for ring in all_rings {
        let mut polygon = Polygon::default();
        for idx in 0..ring.len() {
            let seg = &relation_segments[ring[idx]];
            if idx == 0 {
                polygon.push(seg.node1.id);
            }
            let last_node = polygon[polygon.len() - 1];
            polygon.push(if last_node == seg.node1.id {
                seg.node2.id
            } else {
                seg.node1.id
            });
        }
        polygons.push(polygon);
    }
    Some(polygons)
}

struct SearchParams {
    first_pos: NodePos,
    is_inner: bool,
}

struct ConnectedSegment {
    other_side: NodePos,
    segment_index: usize,
    is_inner: bool,
}

type SegmentConnections = HashMap<NodePos, Vec<ConnectedSegment>>;

fn get_connections(relation_segments: &[NodeDescPair]) -> SegmentConnections {
    let mut connections = SegmentConnections::new();

    for (idx, seg) in relation_segments.iter().enumerate() {
        add_to_connections(&mut connections, seg.node1.pos, seg.node2.pos, idx, seg.is_inner);
        add_to_connections(&mut connections, seg.node2.pos, seg.node1.pos, idx, seg.is_inner);
    }

    connections
}

fn add_to_connections(
    connections: &mut SegmentConnections,
    pos1: NodePos,
    pos2: NodePos,
    segment_index: usize,
    is_inner: bool,
) {
    connections.entry(pos1).or_default().push(ConnectedSegment {
        other_side: pos2,
        segment_index,
        is_inner,
    });
}

struct CurrentRing<'a> {
    available_segments: &'a mut Vec<bool>,
    used_segments: Vec<usize>,
    used_vertices: HashSet<NodePos>,
}

impl<'a> CurrentRing<'a> {
    fn is_valid(&self) -> bool {
        // TODO: check for self-intersections
        true
    }

    fn include_segment(&mut self, seg: &ConnectedSegment) {
        self.available_segments[seg.segment_index] = false;
        self.used_segments.push(seg.segment_index);
        self.used_vertices.insert(seg.other_side);
    }

    fn exclude_segment(&mut self, seg: &ConnectedSegment) {
        self.available_segments[seg.segment_index] = true;
        self.used_segments.pop();
        self.used_vertices.remove(&seg.other_side);
    }
}

fn find_ring(
    relation_segments: &[NodeDescPair],
    connections: &SegmentConnections,
    available_segments: &mut Vec<bool>,
) -> Option<Vec<usize>> {
    for start_idx in 0..available_segments.len() {
        if !available_segments[start_idx] {
            continue;
        }

        available_segments[start_idx] = false;

        {
            let start_segment = &relation_segments[start_idx];
            let mut ring = CurrentRing {
                available_segments,
                used_segments: vec![start_idx],
                used_vertices: [start_segment.node1.pos, start_segment.node2.pos]
                    .into_iter()
                    .cloned()
                    .collect(),
            };
            let search_params = SearchParams {
                first_pos: start_segment.node1.pos,
                is_inner: start_segment.is_inner,
            };

            if find_ring_rec(start_segment.node2.pos, &search_params, connections, &mut ring) {
                return Some(ring.used_segments);
            }
        }

        available_segments[start_idx] = true;
    }

    None
}

fn find_ring_rec(
    last_pos: NodePos,
    search_params: &SearchParams,
    connections: &SegmentConnections,
    ring: &mut CurrentRing,
) -> bool {
    if search_params.first_pos == last_pos && ring.used_segments.len() >= 3 {
        return ring.is_valid();
    }

    let mut candidates = Vec::new();

    match connections.get(&last_pos) {
        None => return false,
        Some(segs) => for seg in segs {
            if seg.is_inner == search_params.is_inner && ring.available_segments[seg.segment_index] {
                candidates.push(seg);
            }
        },
    }

    for cand in candidates {
        if ring.used_vertices.contains(&cand.other_side) && cand.other_side != search_params.first_pos {
            return false;
        }

        ring.include_segment(cand);

        if find_ring_rec(cand.other_side, search_params, connections, ring) {
            return true;
        }

        ring.exclude_segment(cand);
    }

    false
}