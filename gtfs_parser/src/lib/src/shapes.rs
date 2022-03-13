use std::fs::File;
use std::io::{self, prelude::*, BufReader};
use std::fmt;
use std::fs::OpenOptions;
use regex::Regex;

use crate::trips::Trips;
use crate::stop_times::Stoptimes;
use crate::stops::Stops;

#[derive(Debug)]
pub struct Shapes {
    pub shape_id: String,
    pub shape_pt_lat: String,
    pub shape_pt_lon: String,
    pub shape_pt_sequence: String,
    pub shape_dist_traveled: String
}

impl From<&Vec<String>> for Shapes {
    fn from(v: &Vec<String>) -> Self {
        Shapes { shape_id:v[0].to_owned(),
            shape_pt_lat:v[1].to_owned(),
            shape_pt_lon:v[2].to_owned(),
            shape_pt_sequence:v[3].to_owned(),
            shape_dist_traveled:v[4].to_owned() }
    }
}

impl fmt::Display for Shapes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{},{},{},{},{}", self.shape_id, self.shape_pt_lat, self.shape_pt_lon, self.shape_pt_sequence, self.shape_dist_traveled)
    }
}

impl PartialEq for Shapes {
    fn eq(&self, other: &Shapes) -> bool {
        self.shape_id.eq(&other.shape_id) && 
        self.shape_pt_lat.eq(&other.shape_pt_lat) &&
        self.shape_pt_lon.eq(&other.shape_pt_lon) &&
        self.shape_pt_sequence.eq(&other.shape_pt_sequence) &&
        self.shape_dist_traveled.eq(&other.shape_dist_traveled)
    }
}

pub fn read_shapes(path: String) -> io::Result<()> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let kek : String = line?;
        for s in kek.split(" ") {
            print!("{}", s);
        }
        println!();
    }
    Ok(())
}

pub fn build_shapes(stop_times: Vec<Stoptimes>, trips: Vec<Trips>, stops: Vec<Stops>, path: String){
    let mut f = OpenOptions::new().write(true).append(true).create(true).open(path).expect("Unable to open file");
    writeln!(f, "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled").expect("Unable to write file");    
    let mut flip : bool = false;
    let re = Regex::new(r"16:[0-9]{2}:[0-9]{2}").unwrap();
    let mut shapes_id: Vec<i32> = Vec::new();
    for t in trips {
        //if t.route_id.contains("19") {
            if !shapes_id.contains(&t.shape_id.parse::<i32>().unwrap()) {
                shapes_id.push(t.shape_id.parse::<i32>().unwrap());
                println!("the shape_id read -> {}", t.shape_id);
                for s in stop_times.iter().filter(|s| s.trip_id.contains(&t.route_id[..])) {
                    if re.is_match(&s.departure_time[..]) && (s.stop_sequence.parse::<i32>().unwrap() == 1 && flip == false) {
                        flip = true;
                        let stop = stops.iter().filter(|s2| s2.stop_id == s.stop_id).next().unwrap();
                        println!("{},{},{},{},{}", t.shape_id, stop.stop_lat, stop.stop_lon, s.stop_sequence.parse::<i32>().unwrap(), 0);    
                        writeln!(f, "{},{},{},{},{}", t.shape_id, stop.stop_lat, stop.stop_lon, s.stop_sequence.parse::<i32>().unwrap(), 0).expect("Unable to write file");    
                    } else if flip == true {
                        if s.stop_sequence.parse::<i32>().unwrap() == 1 {
                            flip = false;
                            break;
                        } else {
                            let stop = stops.iter().filter(|s2| s2.stop_id == s.stop_id).next().unwrap();
                            println!("{},{},{},{},{}", t.shape_id, stop.stop_lat, stop.stop_lon, s.stop_sequence.parse::<i32>().unwrap(), 0);    
                            writeln!(f, "{},{},{},{},{}", t.shape_id, stop.stop_lat, stop.stop_lon, s.stop_sequence.parse::<i32>().unwrap(), 0).expect("Unable to write file");    
                        }
                    }
                }
                println!();
            }
        }
    //}
}