use crate::draw::point::Point;
use crate::geodata::reader::{Multipolygon, Node, Way};
use crate::tile::coords_to_xy;
use std::cmp::Ordering;
use std::collections::binary_heap::BinaryHeap;

type PointF = (f64, f64);
type LabelPosition = Option<PointF>;

pub trait Labelable {
    fn get_label_position(&self, zoom: u8, scale: f64) -> LabelPosition;
    fn get_waypoints(&self, zoom: u8, scale: f64) -> Option<Vec<Point>>;
}

impl<'n> Labelable for Node<'n> {
    fn get_label_position(&self, zoom: u8, scale: f64) -> LabelPosition {
        let label_position = Point::from_node(self, zoom, scale);
        Some((f64::from(label_position.x), f64::from(label_position.y)))
    }

    fn get_waypoints(&self, _: u8, _: f64) -> Option<Vec<Point>> {
        None
    }
}

impl<'w> Labelable for Way<'w> {
    fn get_label_position(&self, zoom: u8, scale: f64) -> LabelPosition {
        let polygon = nodes_to_points((0..self.node_count()).map(|idx| self.get_node(idx)), zoom, scale);
        get_label_position(&[polygon], scale)
    }

    fn get_waypoints(&self, zoom: u8, scale: f64) -> Option<Vec<Point>> {
        Some(
            (0..self.node_count())
                .map(|idx| Point::from_node(&self.get_node(idx), zoom, scale))
                .collect(),
        )
    }
}

impl<'r> Labelable for Multipolygon<'r> {
    fn get_label_position(&self, zoom: u8, scale: f64) -> LabelPosition {
        let polygons = (0..self.polygon_count())
            .map(|poly_idx| {
                let poly = self.get_polygon(poly_idx);
                nodes_to_points(
                    (0..poly.node_count()).map(|node_idx| poly.get_node(node_idx)),
                    zoom,
                    scale,
                )
            })
            .collect::<Vec<_>>();
        get_label_position(&polygons, scale)
    }

    fn get_waypoints(&self, _: u8, _: f64) -> Option<Vec<Point>> {
        None
    }
}

fn nodes_to_points<'n>(nodes: impl Iterator<Item = Node<'n>>, zoom: u8, scale: f64) -> Vec<PointF> {
    nodes
        .map(|n| {
            let mut coords = coords_to_xy(&n, zoom);
            coords.0 *= scale;
            coords.1 *= scale;
            coords
        })
        .collect()
}

type Polygons<'a> = &'a [Vec<PointF>];

#[derive(Clone)]
struct Cell {
    center: PointF,
    half_cell_size: f64,
    distance_to_center: f64,
    fitness: f64,
    max_fitness: f64,
}

impl Cell {
    fn new<'a>(
        center: &PointF,
        half_cell_size: f64,
        polygons: Polygons<'a>,
        fitness_func: impl Fn(&PointF, f64) -> f64,
    ) -> Cell {
        let distance_to_center = point_to_polygon_dist(center, polygons);
        let max_fitness_distance = distance_to_center + half_cell_size * std::f64::consts::SQRT_2;

        Cell {
            center: *center,
            half_cell_size,
            distance_to_center,
            fitness: fitness_func(center, distance_to_center),
            max_fitness: fitness_func(center, max_fitness_distance),
        }
    }
}

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        self.max_fitness.eq(&other.max_fitness)
    }
}

impl Eq for Cell {}

impl Ord for Cell {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl PartialOrd for Cell {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.max_fitness.partial_cmp(&other.max_fitness)
    }
}

fn polylabel<'a>(polygons: Polygons<'a>, bb: &BoundingBox, precision: f64) -> PointF {
    let size = (bb.width(), bb.height());
    let cell_size = size.0.min(size.1);
    let max_size = size.0.max(size.1);
    let mut half_cell_size = cell_size / 2.0;

    if cell_size == 0.0 {
        return (bb.min_x, bb.min_y);
    }

    let centroid = get_centroid(&polygons[0]);

    let fitness_func = |cell_center: &PointF, distance_polygon: f64| {
        if distance_polygon <= 0.0 {
            return distance_polygon;
        }

        let d = (cell_center.0 - centroid.0, cell_center.1 - centroid.1);
        let distance_centroid = (d.0.powi(2) + d.1.powi(2)).sqrt();
        distance_polygon * (1.0 - distance_centroid / max_size)
    };

    let mut heap = BinaryHeap::new();

    let mut x = bb.min_x;
    while x < bb.max_x {
        let mut y = bb.min_y;
        while y < bb.max_y {
            heap.push(Cell::new(
                &(x + half_cell_size, y + half_cell_size),
                half_cell_size,
                polygons,
                fitness_func,
            ));
            y += cell_size;
        }
        x += cell_size;
    }

    let mut best_cell = Cell::new(&centroid, 0.0, polygons, fitness_func);

    while let Some(current_cell) = heap.pop() {
        if current_cell.fitness > best_cell.fitness {
            best_cell = current_cell.clone();
        }

        if current_cell.max_fitness - best_cell.fitness <= precision {
            continue;
        }

        half_cell_size = current_cell.half_cell_size / 2.0;

        for dx in &[-1.0, 1.0] {
            for dy in &[-1.0, 1.0] {
                let next_center = (
                    current_cell.center.0 + dx * half_cell_size,
                    current_cell.center.1 + dy * half_cell_size,
                );
                heap.push(Cell::new(&next_center, half_cell_size, polygons, fitness_func));
            }
        }
    }

    best_cell.center
}

fn get_label_position(polygons: Polygons<'_>, scale: f64) -> Option<PointF> {
    let _m = crate::perf_stats::measure("Polylabel");

    if polygons.is_empty() || polygons[0].is_empty() {
        return None;
    }

    let bb = get_bounding_box(&polygons[0]);
    let precision = bb.width().max(bb.height()) / 100.0 * scale;

    Some(polylabel(polygons, &bb, precision))
}

struct BoundingBox {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl BoundingBox {
    fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

fn get_bounding_box(polygon: &[PointF]) -> BoundingBox {
    let mut min_x = std::f64::INFINITY;
    let mut max_x = std::f64::NEG_INFINITY;
    let mut min_y = std::f64::INFINITY;
    let mut max_y = std::f64::NEG_INFINITY;

    for point in polygon {
        min_x = min_x.min(point.0);
        max_x = max_x.max(point.0);
        min_y = min_y.min(point.1);
        max_y = max_y.max(point.1);
    }

    BoundingBox {
        min_x,
        max_x,
        min_y,
        max_y,
    }
}

fn segment_dist_sq(point: &PointF, seg_start: &PointF, seg_end: &PointF) -> f64 {
    let mut x = seg_start.0;
    let mut y = seg_start.1;
    let mut dx = seg_end.0 - x;
    let mut dy = seg_end.1 - y;

    if dx != 0.0 || dy != 0.0 {
        let t = ((point.0 - x) * dx + (point.1 - y) * dy) / (dx * dx + dy * dy);

        if t > 1.0 {
            x = seg_end.0;
            y = seg_end.1;
        } else if t > 0.0 {
            x += dx * t;
            y += dy * t;
        }
    }

    dx = point.0 - x;
    dy = point.1 - y;

    dx * dx + dy * dy
}

fn point_to_polygon_dist<'a>(point: &PointF, polygons: Polygons<'a>) -> f64 {
    let mut inside = false;
    let mut min_dist_sq = std::f64::INFINITY;

    for poly in polygons {
        for i in 1..poly.len() {
            let a = &poly[i];
            let b = &poly[i - 1];

            if (a.1 > point.1) != (b.1 > point.1) && (point.0 < (b.0 - a.0) * (point.1 - a.1) / (b.1 - a.1) + a.0) {
                inside = !inside;
            }
            min_dist_sq = min_dist_sq.min(segment_dist_sq(point, a, b));
        }
    }

    let mul = if inside { 1.0 } else { -1.0 };
    mul * min_dist_sq.sqrt()
}

fn get_centroid(polygon: &[PointF]) -> PointF {
    let mut area = 0.0;
    let mut centroid_x = 0.0;
    let mut centroid_y = 0.0;

    for i in 1..polygon.len() {
        let a = &polygon[i];
        let b = &polygon[i - 1];

        let area_component = a.0 * b.1 - b.0 * a.1;
        centroid_x += (a.0 + b.0) * area_component;
        centroid_y += (a.1 + b.1) * area_component;
        area += area_component * 3.0;
    }

    if area == 0.0 {
        polygon[0]
    } else {
        (centroid_x / area, centroid_y / area)
    }
}
