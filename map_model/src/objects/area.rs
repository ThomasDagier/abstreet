use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize, Tags};
use geom::Polygon;
use raw_map::AreaType;

use crate::osm;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AreaID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

impl fmt::Display for AreaID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Area #{}", self.0)
    }
}

/// Areas are just used for drawing.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Area {
    pub id: AreaID,
    pub area_type: AreaType,
    pub polygon: Polygon,
    pub osm_tags: Tags,
    /// The importing process also automatically creates some areas, so they don't come from OSM.
    pub osm_id: Option<osm::OsmID>,
}
