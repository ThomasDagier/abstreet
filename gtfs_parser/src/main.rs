extern crate kek;

use std::io::Write;
use std::fs::OpenOptions;

use kek::agency::*;
use kek::routes::*;
use kek::stop_times::*;
use kek::trips::*;
use kek::stops::*;
use kek::shapes::*;
//use kek::calendar::*;
//use kek::calendar_dates::*;
//use kek::transfers::*;

fn write_content<T>(v: &[T], path: String, first_line: String) where T: std::fmt::Display {
    let mut f = OpenOptions::new().write(true).append(true).create(true).open(path).expect("Unable to open file");
    writeln!(f, "{}", &first_line).expect("Unable to write file");
    for item in v.iter(){
        writeln!(f, "{}", item.to_string()).expect("Unable to write file");    
    }
}

fn main() -> std::io::Result<()> {    
    let names = ["Transports Publics Genevois"];

    let agencies = read_agencies(&names, "./tpg_input/agency.txt".to_string()).unwrap();
    //write_content(&agencies, "./tpg_output/agency.txt".to_string(), "agency_id,agency_name,agency_url,agency_timezone,agency_lang,agency_phone".to_string());

    let routes = read_routes(&agencies, "./tpg_input/routes.txt".to_string()).unwrap();
    //write_content(&routes, "./tpg_output/routes.txt".to_string(), "route_id,agency_id,route_short_name,route_long_name,route_desc,route_type".to_string());

    let stop_times = read_stop_times(&routes, "./tpg_input/stop_times.txt".to_string()).unwrap();
    //write_content(&stop_times, "./tpg_output/stop_times.txt".to_string(), "trip_id,arrival_time,departure_time,stop_id,stop_sequence,pickup_type,drop_off_type".to_string());    

    let stops = read_stops(&stop_times, "./tpg_input/stops.txt".to_string()).unwrap();
    //write_content(&stops, "./tpg_output/stops.txt".to_string(), "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station".to_string());

    let trips = read_trips(&routes, "./tpg_input/trips.txt".to_string()).unwrap();
    //write_content(&trips, "./tpg_output/trips.txt".to_string(), "route_id,service_id,trip_id,trip_headsign,trip_short_name,direction_id,shape_id".to_string());

    //let calendar = read_calendar(&trips, "./tpg_input/calendar.txt".to_string()).unwrap();
    //write_content(&calendar, "./tpg_output/calendar.txt".to_string(), "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date".to_string());

    //let calendar_dates = read_calendar_dates(&trips, "./tpg_input/calendar_dates.txt".to_string()).unwrap();
    //write_content(&calendar_dates, "./tpg_output/calendar_dates.txt".to_string(), "service_id,date,exception_type".to_string());

    //let transfers = read_transfers(&stop_times, "./tpg_input/transfers.txt".to_string()).unwrap();
    //write_content(&transfers, "./tpg_output/transfers.txt".to_string(), "from_stop_id,to_stop_id,transfer_type,min_transfer_time".to_string());
    
    //read_shapes( "./shapes.txt".to_string()).unwrap();
    build_shapes(stop_times, trips, stops, "./tpg_output/shapes.txt".to_string());

    Ok(())
}