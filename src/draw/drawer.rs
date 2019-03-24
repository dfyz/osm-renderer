use crate::errors::*;

use crate::draw::figure::Figure;
use crate::draw::fill::{fill_contour, Filler};
use crate::draw::icon_cache::IconCache;
use crate::draw::labeler::Labeler;
use crate::draw::line::draw_lines;
use crate::draw::png_writer::rgb_triples_to_png;
use crate::draw::point_pairs::PointPairCollection;
use crate::draw::tile_pixels::{dimension, RgbTriples, RgbaColor, TilePixels};
use crate::draw::TILE_SIZE;
use crate::geodata::reader::{Node, OsmEntities, OsmEntity};
use crate::mapcss::styler::{Style, StyledArea, Styler};
use crate::tile::Tile;
use std::path::Path;
use std::sync::Arc;

pub struct Drawer {
    icon_cache: IconCache,
    labeler: Labeler,
}

#[derive(Clone, Eq, PartialEq, Hash)]
enum DrawType {
    Fill,
    Stroke,
    Casing,
}

impl Drawer {
    pub fn new(base_path: &Path) -> Drawer {
        Drawer {
            icon_cache: IconCache::new(base_path),
            labeler: Labeler::default(),
        }
    }

    pub fn draw_tile<'a>(&self, entities: &OsmEntities<'a>, tile: &Tile, styler: &Styler) -> Result<Vec<u8>> {
        let pixels = self.draw_to_pixels(entities, tile, styler);
        rgb_triples_to_png(&pixels, dimension(), dimension())
    }

    pub fn draw_to_pixels<'a>(&self, entities: &OsmEntities<'a>, tile: &Tile, styler: &Styler) -> RgbTriples {
        let mut pixels = TilePixels::new();
        fill_canvas(&mut pixels, styler);

        let styled_areas = {
            let _m = crate::perf_stats::measure("Style areas");
            styler.style_areas(entities.ways.iter(), entities.multipolygons.iter(), tile.zoom)
        };

        let draw_areas_with_type = |pixels: &mut TilePixels, draw_type, use_multipolygons| {
            self.draw_areas(
                pixels,
                &styled_areas,
                tile,
                draw_type,
                use_multipolygons,
                styler.use_caps_for_dashes,
            );
        };

        {
            let _m = crate::perf_stats::measure("Fill areas");
            draw_areas_with_type(&mut pixels, &DrawType::Fill, true);
        }
        {
            let _m = crate::perf_stats::measure("Draw areas");
            draw_areas_with_type(&mut pixels, &DrawType::Casing, false);
            draw_areas_with_type(&mut pixels, &DrawType::Stroke, false);
        }

        let styled_nodes = {
            let _m = crate::perf_stats::measure("Style nodes");
            styler.style_entities(entities.nodes.iter(), tile.zoom)
        };

        {
            let _m = crate::perf_stats::measure("Draw labels");
            self.draw_labels(&mut pixels, tile, &styled_areas, &styled_nodes);
        }

        pixels.to_rgb_triples()
    }

    fn draw_areas(
        &self,
        pixels: &mut TilePixels,
        areas: &[(StyledArea<'_, '_>, Arc<Style>)],
        tile: &Tile,
        draw_type: &DrawType,
        use_multipolygons: bool,
        use_caps_for_dashes: bool,
    ) {
        for (area, style) in areas {
            match area {
                StyledArea::Way(way) => {
                    self.draw_one_area(pixels, tile, *way, style, draw_type, use_caps_for_dashes);
                }
                StyledArea::Multipolygon(rel) if use_multipolygons => {
                    self.draw_one_area(pixels, tile, *rel, style, draw_type, use_caps_for_dashes);
                }
                _ => {}
            }
        }
    }

    fn draw_one_area<'e, A>(
        &self,
        image: &mut TilePixels,
        tile: &Tile,
        area: &'e A,
        style: &Style,
        draw_type: &DrawType,
        use_caps_for_dashes: bool,
    ) where
        A: OsmEntity<'e> + PointPairCollection<'e>,
    {
        let points = area.to_point_pairs(tile.zoom);

        let create_figure = || Figure::new(tile);
        let float_or_one = |num: &Option<f64>| num.unwrap_or(1.0);

        let figure = match *draw_type {
            DrawType::Fill => {
                let opacity = float_or_one(&style.fill_opacity);
                if let Some(ref color) = style.fill_color {
                    let mut figure = create_figure();
                    fill_contour(points, &Filler::Color(color), opacity, &mut figure);
                    Some(figure)
                } else if let Some(ref icon_name) = style.fill_image {
                    let mut figure = create_figure();
                    let read_icon_cache = self.icon_cache.open_read_session(icon_name);
                    if let Some(Some(icon)) = read_icon_cache.get(icon_name) {
                        fill_contour(points, &Filler::Image(icon), opacity, &mut figure);
                    }
                    Some(figure)
                } else {
                    None
                }
            }
            DrawType::Casing => style.casing_color.as_ref().and_then(|color| {
                let mut figure = create_figure();
                style.casing_width.map(|casing_width| {
                    draw_lines(
                        points,
                        casing_width,
                        color,
                        1.0,
                        &style.casing_dashes,
                        &style.casing_line_cap,
                        use_caps_for_dashes,
                        &mut figure,
                    );
                    figure
                })
            }),
            DrawType::Stroke => style.color.as_ref().map(|color| {
                let mut figure = create_figure();
                draw_lines(
                    points,
                    float_or_one(&style.width),
                    color,
                    float_or_one(&style.opacity),
                    &style.dashes,
                    &style.line_cap,
                    use_caps_for_dashes,
                    &mut figure,
                );
                figure
            }),
        };

        if let Some(ref figure) = figure {
            draw_figure(figure, image, tile);
        }
    }

    fn draw_labels(
        &self,
        image: &mut TilePixels,
        tile: &Tile,
        areas: &[(StyledArea<'_, '_>, Arc<Style>)],
        nodes: &[(&Node<'_>, Arc<Style>)],
    ) {
        let mut all_labels_figure = Figure::new(tile);

        {
            let _m = crate::perf_stats::measure("Label areas");
            for &(ref area, ref style) in areas {
                match area {
                    StyledArea::Way(way) => {
                        self.labeler
                            .label_entity(*way, style, tile.zoom, &self.icon_cache, &mut all_labels_figure)
                    }
                    StyledArea::Multipolygon(rel) => {
                        self.labeler
                            .label_entity(*rel, style, tile.zoom, &self.icon_cache, &mut all_labels_figure)
                    }
                }
            }
        }

        {
            let _m = crate::perf_stats::measure("Label nodes");
            for &(node, ref style) in nodes {
                self.labeler
                    .label_entity(node, style, tile.zoom, &self.icon_cache, &mut all_labels_figure);
            }
        }

        let _m = crate::perf_stats::measure("Draw figure with labels");
        draw_figure(&all_labels_figure, image, tile);
    }
}

fn fill_canvas(image: &mut TilePixels, styler: &Styler) {
    if let Some(ref c) = styler.canvas_fill_color {
        let canvas_rgba = RgbaColor::from_color(c, 1.0);
        for x in 0..TILE_SIZE {
            for y in 0..TILE_SIZE {
                image.set_pixel(x, y, &canvas_rgba);
            }
        }
    }
}

fn draw_figure(figure: &Figure, image: &mut TilePixels, tile: &Tile) {
    let to_tile_start = |c| (c as usize) * TILE_SIZE;
    let (tile_start_x, tile_start_y) = (to_tile_start(tile.x), to_tile_start(tile.y));

    for (y, x_to_color) in figure.pixels.range(tile_start_y..(tile_start_y + TILE_SIZE)) {
        let real_y = *y - tile_start_y;
        for (x, color) in x_to_color.range(tile_start_x..(tile_start_x + TILE_SIZE)) {
            let real_x = *x - tile_start_x;
            image.set_pixel(real_x, real_y, color);
        }
    }
}
