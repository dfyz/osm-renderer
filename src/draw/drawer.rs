use errors::*;

use geodata::reader::OsmEntities;
use mapcss::styler::Styler;
use tile;

pub trait Drawer {
    fn draw_tile(
        &self,
        entities: &OsmEntities,
        tile: &tile::Tile,
        styler: &Styler,
    ) -> Result<Vec<u8>>;
}
