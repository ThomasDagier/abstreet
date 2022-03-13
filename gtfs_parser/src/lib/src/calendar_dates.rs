use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::fmt;
use crate::trips::Trips;

#[derive(Debug)]
pub struct Calendardates {
    pub service_id: String,
    pub date: String,
    pub exception_type: String
}

impl From<&Vec<String>> for Calendardates {
    fn from(v: &Vec<String>) -> Self {
        Calendardates { service_id:v[0].to_owned(),
            date:v[1].to_owned(),
            exception_type:v[2].to_owned() }
    }
}

impl fmt::Display for Calendardates {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\",\"{}\",\"{}\"", self.service_id, self.date, self.exception_type)
    }
}

impl PartialEq for Calendardates {
    fn eq(&self, other: &Calendardates) -> bool {
        self.service_id.eq(&other.service_id) && 
        self.date.eq(&other.date) &&
        self.exception_type.eq(&other.exception_type)
    }
}

pub fn read_calendar_dates(trips: &Vec<Trips>, path: String) -> Option<Vec<Calendardates>> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut calendar_dates = reader.lines().into_iter().map(|line| {
        let v = line.unwrap().split(",").map(|word| word.replace("\"", "")).collect::<Vec<_>>();
        trips.iter().filter(|trip| v[0] == trip.service_id).map(|_service_id| Calendardates::from(&v)).collect::<Vec<_>>()
    }).flatten().collect::<Vec<_>>();
    calendar_dates.dedup();
    Some(calendar_dates)
}