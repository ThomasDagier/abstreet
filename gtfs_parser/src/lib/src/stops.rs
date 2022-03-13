use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::fmt;
use crate::stop_times::{Stoptimes};

#[derive(Debug)]
pub struct Stops {
    pub stop_id: String,
    pub stop_name: String,
    pub stop_lat: String,
    pub stop_lon: String,
    pub location_type: String,
    pub parent_station: String
}

impl From<&Vec<String>> for Stops {
    fn from(v: &Vec<String>) -> Self {
        Stops { stop_id:v[0].to_owned(),
            stop_name:v[1].to_owned(),
            stop_lat:v[2].to_owned(),
            stop_lon:v[3].to_owned(),
            location_type:v[4].to_owned(),
            parent_station:v[5].to_owned() }
    }
}

impl fmt::Display for Stops {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{},\"{}\",{},{},{},{}", self.stop_id, self.stop_name, self.stop_lat, self.stop_lon, self.location_type, self.parent_station)
    }
}

impl PartialEq for Stops {
    fn eq(&self, other: &Stops) -> bool {
        self.stop_id.eq(&other.stop_id) && 
        self.stop_name.eq(&other.stop_name) &&
        self.stop_lat.eq(&other.stop_lat) &&
        self.stop_lon.eq(&other.stop_lon) &&
        self.location_type.eq(&other.location_type) &&
        self.parent_station.eq(&other.parent_station)
    }
}

pub fn read_stops(stop_times: &Vec<Stoptimes>, path: String) -> Option<Vec<Stops>> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut stops = reader.lines().into_iter().map(|line| {
        let v = line.unwrap().split(",").map(|word| word.replace("\"", "")).collect::<Vec<_>>();
        stop_times.iter().filter(|stop_time| v[0] == stop_time.stop_id).map(|_stop_id| Stops::from(&v)).collect::<Vec<_>>()
    }).flatten().collect::<Vec<_>>();
    stops.dedup();
    Some(stops)
}