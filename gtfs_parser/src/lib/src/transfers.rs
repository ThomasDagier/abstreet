use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::fmt;
use crate::stop_times::{Stoptimes};

#[derive(Debug)]
pub struct Transfers {
    pub from_stop_id: String,
    pub to_stop_id: String,
    pub transfer_type: String,
    pub min_transfer_time: String
}

impl From<&Vec<String>> for Transfers {
    fn from(v: &Vec<String>) -> Self {
        Transfers { from_stop_id:v[0].to_owned(),
            to_stop_id:v[1].to_owned(),
            transfer_type:v[2].to_owned(),
            min_transfer_time:v[3].to_owned() }
    }
}

impl fmt::Display for Transfers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\",\"{}\",\"{}\",\"{}\"", self.from_stop_id, self.to_stop_id, self.transfer_type, self.min_transfer_time)
    }
}

impl PartialEq for Transfers {
    fn eq(&self, other: &Transfers) -> bool {
        self.from_stop_id.eq(&other.from_stop_id) && 
        self.to_stop_id.eq(&other.to_stop_id) &&
        self.transfer_type.eq(&other.transfer_type) &&
        self.min_transfer_time.eq(&other.min_transfer_time)
    }
}

pub fn read_transfers(stop_times: &Vec<Stoptimes>, path: String) -> Option<Vec<Transfers>> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut transfers = reader.lines().into_iter().map(|line| {
        let v = line.unwrap().split(",").map(|word| word.replace("\"", "")).collect::<Vec<_>>();
        // is there a small chance that this stop doesn't bellong to the current agency ? 
        // --> to be sure we test either the field "from_stop_id" or "to_stop_id" to match with each "stop_id" from Vec<Stoptimes>
        stop_times.iter().filter(|stop_time| (v[0] == stop_time.stop_id || v[1] == stop_time.stop_id)).map(|_stop_id| Transfers::from(&v)).collect::<Vec<_>>()
    }).flatten().collect::<Vec<_>>();
    transfers.dedup();
    Some(transfers)
}