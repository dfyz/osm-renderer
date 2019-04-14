use crate::draw::fill::{fill_contour, Filler};
use crate::draw::icon_cache::IconCache;
use crate::draw::labeler::Labeler;
use crate::draw::line::draw_lines;
use crate::draw::png_writer::rgb_triples_to_png;
use crate::draw::point_pairs::PointPairCollection;
use crate::draw::tile_pixels::{RgbTriples, RgbaColor, TilePixels};
use crate::geodata::reader::{Node, OsmEntities, OsmEntity};
use crate::mapcss::styler::{Style, StyledArea, Styler, TextPosition};
use crate::tile::Tile;
use failure::Error;
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

pub struct TileRenderedPixels {
    pub triples: RgbTriples,
    pub dimension: usize,
}

impl Drawer {
    pub fn new(base_path: &Path) -> Drawer {
        Drawer {
            icon_cache: IconCache::new(base_path),
            labeler: Labeler::default(),
        }
    }

    pub fn draw_tile<'a>(
        &self,
        entities: &OsmEntities<'a>,
        tile: &Tile,
        scale: usize,
        styler: &Styler,
    ) -> Result<Vec<u8>, Error> {
        let pixels = self.draw_to_pixels(entities, tile, scale, styler);
        rgb_triples_to_png(&pixels.triples, pixels.dimension, pixels.dimension)
    }

    pub fn draw_to_pixels<'a>(
        &self,
        entities: &OsmEntities<'a>,
        tile: &Tile,
        scale: usize,
        styler: &Styler,
    ) -> TileRenderedPixels {
        let mut pixels = TilePixels::new(tile, scale);
        fill_canvas(&mut pixels, styler);

        let styled_areas = {
            let _m = crate::perf_stats::measure("Style areas");
            styler.style_areas(entities.ways.iter(), entities.multipolygons.iter(), tile.zoom, false)
        };

        let float_scale = scale as f64;

        let draw_areas_with_type = |pixels: &mut TilePixels, draw_type, use_multipolygons| {
            self.draw_areas(
                pixels,
                &styled_areas,
                tile.zoom,
                float_scale,
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

        pixels.blend_unfinished_pixels(false);

        let styled_areas_for_labels = {
            let _m = crate::perf_stats::measure("Style area for labels");
            styler.style_areas(entities.ways.iter(), entities.multipolygons.iter(), tile.zoom, true)
        };

        let styled_nodes = {
            let _m = crate::perf_stats::measure("Style nodes");
            styler.style_entities(entities.nodes.iter(), tile.zoom, true)
        };

        {
            let _m = crate::perf_stats::measure("Draw labels");
            self.draw_labels(
                &mut pixels,
                tile.zoom,
                float_scale,
                &styled_areas_for_labels,
                &styled_nodes,
            );
        }

        pixels.blend_unfinished_pixels(true);

        TileRenderedPixels {
            triples: pixels.to_rgb_triples(),
            dimension: pixels.dimension(),
        }
    }

    fn draw_areas(
        &self,
        pixels: &mut TilePixels,
        areas: &[(StyledArea<'_, '_>, Arc<Style>)],
        zoom: u8,
        scale: f64,
        draw_type: &DrawType,
        use_multipolygons: bool,
        use_caps_for_dashes: bool,
    ) {
        for (area, style) in areas {
            match area {
                StyledArea::Way(way) => {
                    self.draw_one_area(pixels, zoom, scale, *way, style, draw_type, use_caps_for_dashes);
                }
                StyledArea::Multipolygon(rel) if use_multipolygons => {
                    self.draw_one_area(pixels, zoom, scale, *rel, style, draw_type, use_caps_for_dashes);
                }
                _ => {}
            }
        }
    }

    fn draw_one_area<'e, A>(
        &self,
        pixels: &mut TilePixels,
        zoom: u8,
        scale: f64,
        area: &'e A,
        style: &Style,
        draw_type: &DrawType,
        use_caps_for_dashes: bool,
    ) where
        A: OsmEntity<'e> + PointPairCollection<'e>,
    {
        let points = area.to_point_pairs(zoom, scale);
        let float_or_one = |num: &Option<f64>| num.unwrap_or(1.0);

        let scale_dashes =
            |dashes: &Option<Vec<f64>>| dashes.as_ref().map(|nums| nums.iter().map(|x| x * scale).collect());

        match *draw_type {
            DrawType::Fill => {
                let opacity = float_or_one(&style.fill_opacity);
                if let Some(ref color) = style.fill_color {
                    fill_contour(points, &Filler::Color(color), opacity, pixels);
                } else if let Some(ref icon_name) = style.fill_image {
                    let read_icon_cache = self.icon_cache.open_read_session(icon_name);
                    if let Some(Some(icon)) = read_icon_cache.get(icon_name) {
                        fill_contour(points, &Filler::Image(icon), opacity, pixels);
                    }
                }
            }
            DrawType::Casing => {
                if let Some(color) = style.casing_color.as_ref() {
                    if let Some(casing_width) = style.casing_width {
                        draw_lines(
                            points,
                            casing_width * scale,
                            color,
                            1.0,
                            &scale_dashes(&style.casing_dashes),
                            &style.casing_line_cap,
                            use_caps_for_dashes,
                            pixels,
                        );
                    }
                }
            }
            DrawType::Stroke => {
                if let Some(color) = style.color.as_ref() {
                    draw_lines(
                        points,
                        scale * float_or_one(&style.width),
                        color,
                        float_or_one(&style.opacity),
                        &scale_dashes(&style.dashes),
                        &style.line_cap,
                        use_caps_for_dashes,
                        pixels,
                    );
                }
            }
        }

        pixels.bump_generation();
    }

    fn draw_labels(
        &self,
        pixels: &mut TilePixels,
        zoom: u8,
        scale: f64,
        areas: &[(StyledArea<'_, '_>, Arc<Style>)],
        nodes: &[(&Node<'_>, Arc<Style>)],
    ) {
        {
            let _m = crate::perf_stats::measure("Label areas");
            for &(ref area, ref style) in areas {
                match area {
                    StyledArea::Way(way) => self.labeler.label_entity(
                        *way,
                        style,
                        zoom,
                        scale,
                        &self.icon_cache,
                        TextPosition::Line,
                        pixels,
                    ),
                    StyledArea::Multipolygon(rel) => self.labeler.label_entity(
                        *rel,
                        style,
                        zoom,
                        scale,
                        &self.icon_cache,
                        TextPosition::Center,
                        pixels,
                    ),
                }
            }
        }

        {
            let _m = crate::perf_stats::measure("Label nodes");
            for &(node, ref style) in nodes {
                self.labeler
                    .label_entity(node, style, zoom, scale, &self.icon_cache, TextPosition::Center, pixels);
            }
        }
    }
}

fn fill_canvas(pixels: &mut TilePixels, styler: &Styler) {
    if let Some(ref c) = styler.canvas_fill_color {
        pixels.fill(&RgbaColor::from_color(c, 1.0));
    }
}
