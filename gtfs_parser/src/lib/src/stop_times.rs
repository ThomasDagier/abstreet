use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::fmt;
use crate::routes::Routes;

#[derive(Debug)]
pub struct Stoptimes {
    pub trip_id: String,
    pub arrival_time: String,
    pub departure_time: String,
    pub stop_id: String,
    pub stop_sequence: String,
    pub pickup_type: String,
    pub drop_off_type: String
}

impl From<&Vec<String>> for Stoptimes {
    fn from(v: &Vec<String>) -> Self {
        Stoptimes { trip_id:v[0].to_owned(),
            arrival_time:v[1].to_owned(),
            departure_time:v[2].to_owned(),
            stop_id:v[3].to_owned(),
            stop_sequence:v[4].to_owned(),
            pickup_type:v[5].to_owned(),
            drop_off_type:v[6].to_owned() }
    }
}

impl fmt::Display for Stoptimes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\",\"{}\",\"{}\",\"{}\",{},{},{}", self.trip_id, self.arrival_time, self.departure_time, self.stop_id, self.stop_sequence, self.pickup_type, self.drop_off_type)
    }
}

impl PartialEq for Stoptimes {
    fn eq(&self, other: &Stoptimes) -> bool {
        self.trip_id.eq(&other.trip_id) && 
        self.arrival_time.eq(&other.arrival_time) &&
        self.departure_time.eq(&other.departure_time) &&
        self.stop_id.eq(&other.stop_id) &&
        self.stop_sequence.eq(&other.stop_sequence) &&
        self.pickup_type.eq(&other.pickup_type) &&
        self.drop_off_type.eq(&other.drop_off_type)
    }
}

pub fn read_stop_times(routes: &Vec<Routes>, path: String) -> Option<Vec<Stoptimes>> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut stop_times = reader.lines().into_iter().map(|line| {
        let v = line.unwrap().split(",").map(|word| word.replace("\"", "")).collect::<Vec<_>>();
        routes.iter().filter(|route| v[0].split(".").collect::<Vec<_>>().contains(&&(route.route_id)[..])).map(|_route_id| Stoptimes::from(&v)).collect::<Vec<_>>()
    }).flatten().collect::<Vec<_>>();
    stop_times.dedup();
    Some(stop_times)
}
