use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;

use anyhow::Result;
use serde::Deserialize;

use abstutil::MultiMap;
use geom::{LonLat, PolyLine, Pt2D};
use kml::{ExtraShape, ExtraShapes};
use map_model::raw::{RawMap, RawTransitRoute, RawTransitStop};
use map_model::PathConstraints;

pub fn import(map: &mut RawMap) -> Result<()> {
    // Collect metadata about routes
    for rec in csv::Reader::from_reader(File::open(map.name.city.input_path("gtfs/routes.txt"))?)
        .deserialize()
    {
        let rec: Route = rec?;
        // See https://developers.google.com/transit/gtfs/reference#routestxt
        let route_type = match rec.route_type {
            3 => PathConstraints::Bus,
            // These aren't distinguished in the map model yet. Trams and streetcars might
            // particularly mess up...  or just fail to snap to a road later.
            0 | 1 | 2 => PathConstraints::Train,
            _ => continue,
        };
        map.transit_routes.push(RawTransitRoute {
            long_name: if rec.route_long_name.is_empty() {
                rec.route_desc
            } else {
                rec.route_long_name
            },
            short_name: rec.route_short_name,
            gtfs_id: rec.route_id.0,
            shape: PolyLine::dummy(),
            stops: Vec::new(),
            route_type,
        });
    }

    // Map route_id to shape_id
    let mut route_to_shapes = MultiMap::new();
    // Map (route_id, shape_id) to trip_id
    let mut route_and_shape_to_trips = MultiMap::new();
    for rec in csv::Reader::from_reader(File::open(map.name.city.input_path("gtfs/trips.txt"))?)
        .deserialize()
    {
        let rec: Trip = rec?;
        route_to_shapes.insert(rec.route_id.clone(), rec.shape_id.clone());
        route_and_shape_to_trips.insert((rec.route_id, rec.shape_id), rec.trip_id);
    }

    
    // Scrape all shape data. Map from shape_id to points and the sequence number
    let mut raw_shapes: HashMap<ShapeID, Vec<(Pt2D, usize)>> = HashMap::new();
    for rec in csv::Reader::from_reader(File::open(map.name.city.input_path("gtfs/shapes.txt"))?)
    .deserialize()
    {
        let rec: Shape = rec?;
        let pt = LonLat::new(rec.shape_pt_lon, rec.shape_pt_lat).to_pt(&map.gps_bounds);
        raw_shapes
            .entry(rec.shape_id)
            .or_insert_with(Vec::new)
            .push((pt, rec.shape_pt_sequence));
    }

    //println!("{}", route_to_shapes.len());
    //println!("{}", route_and_shape_to_trips.len());
    //std::process::exit(0);

    // TRY TO MAKE IT WORK FOR LINE 19

    // Build a PolyLine for every route
    let mut transit_routes = Vec::new();
    let mut route_to_shape = HashMap::new();
    for mut route in map.transit_routes.drain(..) {
        println!("the current route : {:?}", route);
        let shape_ids = route_to_shapes.get(RouteID(route.gtfs_id.clone()));
        if shape_ids.is_empty() {
            warn!("Route {} has no shape", route.gtfs_id);
            continue;
        }
        if shape_ids.len() > 1 {
            warn!(
                //"Route {} has several shapes, choosing one arbitrarily: {:?}",
                //route.gtfs_id, shape_ids
                "Route {} has {} shapes", route.gtfs_id, shape_ids.len()
            );
        }
        let shape_id = shape_ids.into_iter().next().unwrap();
        println!("id taken from the list of shapes -> {}", shape_id.0);
        route_to_shape.insert(RouteID(route.gtfs_id.clone()), shape_id.clone());
        let mut pts = if let Some(pts) = raw_shapes.remove(shape_id) {
            pts
        } else {
            warn!("Route {} is missing its shape", route.gtfs_id);
            continue;
        };
        // Points are usually sorted, but just in case...
        pts.sort_by_key(|(_, seq)| *seq);
        let pts: Vec<Pt2D> = pts.into_iter().map(|(pt, _)| pt).collect();
        match PolyLine::new(pts) {
            Ok(pl) => {
                route.shape = pl;
                transit_routes.push(route);
            }
            Err(err) => {
                warn!("Route {} has a weird shape: {}", route.gtfs_id, err);
                continue;
            }
        }
    }

    // For now, every route uses exactly one trip ID, and there's no schedule. Just pick an
    // arbitrary trip per route.
    let mut route_to_trip = HashMap::new();
    for (route_id, shape_id) in &route_to_shape {
        let trips = route_and_shape_to_trips.get((route_id.clone(), shape_id.clone()));
        if let Some(trip_id) = trips.into_iter().next() {
            route_to_trip.insert(route_id.clone(), trip_id);
        }
    }

    // Scrape the trip ID -> (stop ID, sequence number)
    let mut trip_to_stops: HashMap<TripID, Vec<(StopID, usize)>> = HashMap::new();
    for rec in
        csv::Reader::from_reader(File::open(map.name.city.input_path("gtfs/stop_times.txt"))?)
            .deserialize()
    {
        let rec: StopTime = rec?;
        trip_to_stops
            .entry(rec.trip_id)
            .or_insert_with(Vec::new)
            .push((rec.stop_id, rec.stop_sequence));
    }

    // Assign the stops for every route
    let mut stop_ids = HashSet::new();
    for route in &mut map.transit_routes {
        let trip_id = route_to_trip[&RouteID(route.gtfs_id.clone())];
        let mut stops = trip_to_stops.remove(&trip_id).unwrap_or_else(Vec::new);
        stops.sort_by_key(|(_, seq)| *seq);
        for (stop_id, _) in stops {
            route.stops.push(stop_id.0.clone());
            stop_ids.insert(stop_id);
        }
    }

    // Scrape stop metadata
    for rec in csv::Reader::from_reader(File::open(map.name.city.input_path("gtfs/stops.txt"))?)
        .deserialize()
    {
        let rec: Stop = rec?;
        if stop_ids.contains(&rec.stop_id) {
            let position = LonLat::new(rec.stop_lon, rec.stop_lat).to_pt(&map.gps_bounds);
            if map.boundary_polygon.contains_pt(position) {
                map.transit_stops.insert(
                    rec.stop_id.0.clone(),
                    RawTransitStop {
                        gtfs_id: rec.stop_id.0,
                        position,
                        name: rec.stop_name,
                    },
                );
            }
        }
    }

    // Make sure all of the stops are valid and used by some route
    let mut used_stops = HashSet::new();
    for route in &mut map.transit_routes {
        route.stops.retain(|stop_id| {
            used_stops.insert(stop_id.clone());
            map.transit_stops.contains_key(stop_id)
        });
    }
    map.transit_routes.retain(|route| !route.stops.is_empty());
    map.transit_stops
        .retain(|stop_id, _| used_stops.contains(stop_id));

    if false {
        dump_kml(map);
    }

    // print all data to see if gtfs import worked
    map.transit_routes = transit_routes;
    println!("Result:");
    for lol in map.transit_routes.iter() {
        println!("{:?}", lol);
        println!("\n");
    }
    //std::process::exit(0);

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
struct ShapeID(String);
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
struct TripID(String);
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
struct StopID(String);
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
struct RouteID(String);

#[derive(Deserialize)]
struct Route {
    route_id: RouteID,
    route_short_name: String,
    route_long_name: String,
    route_desc: String,
    route_type: usize,
}

#[derive(Deserialize)]
struct Trip {
    route_id: RouteID,
    shape_id: ShapeID,
    trip_id: TripID,
}

#[derive(Deserialize, Debug)]
struct Shape {
    shape_id: ShapeID,
    shape_pt_lat: f64,
    shape_pt_lon: f64,
    shape_pt_sequence: usize,
}

#[derive(Deserialize)]
struct Stop {
    stop_id: StopID,
    stop_lon: f64,
    stop_lat: f64,
    stop_name: String,
}

#[derive(Deserialize)]
struct StopTime {
    trip_id: TripID,
    stop_id: StopID,
    stop_sequence: usize,
}

fn dump_kml(map: &RawMap) {
    let mut shapes = Vec::new();

    // One polyline per route
    for route in &map.transit_routes {
        let points = map.gps_bounds.convert_back(route.shape.points());
        let mut attributes = BTreeMap::new();
        attributes.insert("long_name".to_string(), route.long_name.clone());
        attributes.insert("short_name".to_string(), route.short_name.clone());
        attributes.insert("gtfs_id".to_string(), route.gtfs_id.clone());
        attributes.insert("num_stops".to_string(), route.stops.len().to_string());
        attributes.insert("route_type".to_string(), format!("{:?}", route.route_type));
        shapes.push(ExtraShape { points, attributes });
    }

    // One point per stop
    for stop in map.transit_stops.values() {
        let mut attributes = BTreeMap::new();
        attributes.insert("gtfs_id".to_string(), stop.gtfs_id.clone());
        attributes.insert("name".to_string(), stop.name.clone());
        let points = vec![stop.position.to_gps(&map.gps_bounds)];
        shapes.push(ExtraShape { points, attributes });
    }

    abstio::write_binary(
        map.name
            .city
            .input_path(format!("gtfs_{}.bin", map.name.map)),
        &ExtraShapes { shapes },
    );
}


/*
0, 46.1761347309608, 6.13265471509219, 1, 0
0, 46.1798668011167, 6.13839494975851, 2, 0
0, 46.1812165041179, 6.14082938417881, 3, 0
0, 46.1843325121451, 6.14038920968953, 4, 0
0, 46.1868015587535, 6.14020954663268, 5, 0
0, 46.1888227360204, 6.14303923977805, 6, 0
0, 46.1918388161971, 6.14400043713219, 7, 0
0, 46.1948982583221, 6.14342551535028, 8, 0
0, 46.198144065585, 6.14316500391785, 9, 0
0, 46.2011969392001, 6.14418010018904, 10, 0
0, 46.2047283614253, 6.1430482229309, 11, 0
0, 46.2031118935082, 6.14718047323842, 12, 0
0, 46.2018622068667, 6.15281291007063, 13, 0
0, 46.2010788067161, 6.15641515436045, 14, 0
0, 46.2006187093576, 6.15854416158411, 15, 0
0, 46.2000839967758, 6.16452694137718, 16, 0
0, 46.2000093853042, 6.16768002802488, 17, 0
0, 46.1996114557443, 6.1746689209363, 18, 0
0, 46.1988342412615, 6.18036423983841, 19, 0
0, 46.1982746400266, 6.18563735055693, 20, 0
0, 46.1953646217344, 6.19247352987003, 21, 0
0, 46.1939779562442, 6.1965518812605, 22, 0
0, 46.1932379724497, 6.20090871038909, 23, 0
0, 46.1924482308546, 6.20646928199856, 24, 0


FAUT ENLEVER TOUS CES PUTAIN D'ESPACES 

*/