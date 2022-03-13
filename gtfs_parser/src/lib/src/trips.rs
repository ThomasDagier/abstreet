use std::borrow::Borrow;
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::fmt;
use crate::routes::Routes;

#[derive(Debug)]
pub struct Trips {
    pub route_id: String,
    pub service_id: String,
    pub trip_id: String,
    pub trip_headsign: String,
    pub trip_short_name: String,
    pub direction_id: String,
    pub shape_id: String
}

impl From<&Vec<String>> for Trips {
    fn from(v: &Vec<String>) -> Self {
        Trips { route_id:v[0].to_owned(),
            service_id:v[1].to_owned(),
            trip_id:v[2].to_owned(),
            trip_headsign:v[3].to_owned(),
            trip_short_name:v[4].to_owned(),
            direction_id:v[5].to_owned(),
            shape_id:v[6].to_owned() }
    }
}

impl fmt::Display for Trips {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{}", self.route_id, self.service_id, self.trip_id, self.trip_headsign, self.trip_short_name, self.direction_id, self.shape_id)
    }
}

impl PartialEq for Trips {
    fn eq(&self, other: &Trips) -> bool {
        self.route_id.eq(&other.route_id) && 
        self.service_id.eq(&other.service_id) &&
        self.trip_id.eq(&other.trip_id) &&
        self.trip_headsign.eq(&other.trip_headsign) &&
        self.trip_short_name.eq(&other.trip_short_name) &&
        self.direction_id.eq(&other.direction_id) &&
        self.shape_id.eq(&other.shape_id)
    }
}

pub fn read_trips(routes: &Vec<Routes>, path: String) -> Option<Vec<Trips>> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut trips = reader.lines().into_iter().map(|line| {
        let v = line.unwrap().split(",").map(|word| word.replace("\"", "")).collect::<Vec<_>>();
        routes.iter().filter(|route| v[0].split(".").collect::<Vec<_>>().contains(&&(route.route_id)[..])).map(|_route_id| Trips::from(&v)).collect::<Vec<_>>()
    }).flatten().collect::<Vec<_>>();
    trips.dedup();

    let len = trips.len();
    for i in 0..len {
        for j in 0..len {
            if trips[i].route_id == trips[j].route_id && trips[i].trip_headsign == trips[j].trip_headsign && trips[i].trip_short_name == trips[j].trip_short_name && i != j {
                trips[j].shape_id = (&trips[i].shape_id).to_string();
                //println!("{}: {:?}, {}: {:?}", i, trips[i], j, trips[j]);
            }
        }
    }

    Some(trips)
}