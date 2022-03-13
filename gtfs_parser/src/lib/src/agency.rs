use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::fmt;

pub struct Agency {
    pub agency_id: String,
    pub agency_name: String,
    pub agency_url: String,
    pub agency_timezone: String,
    pub agency_lang: String,
    pub agency_phone: String
}

impl From<&Vec<String>> for Agency {
    fn from(v: &Vec<String>) -> Self {
        Agency {agency_id:v[0].to_owned(), 
            agency_name:v[1].to_owned(),
            agency_url:v[2].to_owned(),
            agency_timezone:v[3].to_owned(), 
            agency_lang:v[4].to_owned(), 
            agency_phone:v[5].to_owned() }
    }
}

impl fmt::Display for Agency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"", self.agency_id, self.agency_name, self.agency_url, self.agency_timezone, self.agency_lang, self.agency_phone)
    }
}

impl PartialEq for Agency {
    fn eq(&self, other: &Agency) -> bool {
        self.agency_id.eq(&other.agency_id) && 
        self.agency_name.eq(&other.agency_name) &&
        self.agency_url.eq(&other.agency_url) &&
        self.agency_timezone.eq(&other.agency_timezone) &&
        self.agency_lang.eq(&other.agency_lang) &&
        self.agency_phone.eq(&other.agency_phone)
    }
}

// return all agencies matching with the names in the list given
pub fn read_agencies(names: &[&str], path: String) -> Option<Vec<Agency>> {
    // open the file (might fail so we call ".ok()?" to check if open() worked fine)
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    // we create the Vec<Agency> from each lines we read from the BufReader
    let mut agencies = reader.lines().into_iter().map(|line| {
        // for each line we store the data in a Vec<String> containing every word separated by a coma and formated (removed the \")
        let v = line.unwrap().split(",").map(|word| word.replace("\"", "")).collect::<Vec<_>>();
        // then, we create an agency only if "agency_id" from the current line (so v[1] from the Vec<String>) is matching with any name from the array given (by using "filter()")
        // the new agency is stored in a Vec<Vec<Agency>> as their might be several names in the array given (for a line, we could create multiple agencies at the same time).
        // consquently, we have to flatten the Vec<Vec<Agency>> in a Vec<Agency>.
        // then, we collecte the flattened Vec so we can store it into "agencies" that will be return in an option
        names.iter().filter(|name| name.contains(&&(v[1])[..])).map(|_agency_name| Agency::from(&v)).collect::<Vec<_>>()
    }).flatten().collect::<Vec<_>>();
    // we return Some(...), an option on a Vec<Agency> as the function "read_agencies()" might fail while calling the open() function
    agencies.dedup();
    Some(agencies)
}