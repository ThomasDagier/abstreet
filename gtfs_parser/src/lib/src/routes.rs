use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::fmt;
use crate::agency::Agency;

#[derive(Debug)] // so we can use {:?} to print datas
pub struct Routes {
    pub route_id: String,
    pub agency_id: String,
    pub route_short_name: String,
    pub route_long_name: String,
    pub route_desc: String,
    pub route_type: String
}

impl From<&Vec<String>> for Routes {
    fn from(v: &Vec<String>) -> Self {
        Routes { route_id:v[0].to_owned(),
            agency_id:v[1].to_owned(),
            route_short_name:v[2].to_owned(),
            route_long_name:v[3].to_owned(),
            route_desc:v[4].to_owned(),
            route_type:v[5].to_owned() }
    }
}

impl fmt::Display for Routes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // overwrite route_type as Dustin expect it to be 3 and not 700 / 900
        write!(f, "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",3", self.route_id, self.agency_id, self.route_short_name, self.route_long_name, self.route_desc)
        //write!(f, "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{}", self.route_id, self.agency_id, self.route_short_name, self.route_long_name, self.route_desc, self.route_type)
    }
}

impl PartialEq for Routes {
    fn eq(&self, other: &Routes) -> bool {
        self.route_id.eq(&other.route_id) && 
        self.agency_id.eq(&other.agency_id) &&
        self.route_short_name.eq(&other.route_short_name) &&
        self.route_long_name.eq(&other.route_long_name) &&
        self.route_desc.eq(&other.route_desc) &&
        self.route_type.eq(&other.route_type)
    }
}

pub fn read_routes(agencies: &Vec<Agency>, path: String) -> Option<Vec<Routes>> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut routes = reader.lines().into_iter().map(|line| {
        let v = line.unwrap().split(",").map(|word| word.replace("\"", "")).collect::<Vec<_>>();
        agencies.iter().filter(|agency| agency.agency_id == v[1]).map(|_agency_id| Routes::from(&v)).collect::<Vec<_>>()
    }).flatten().collect::<Vec<_>>();
    routes.dedup();
    Some(routes)
}