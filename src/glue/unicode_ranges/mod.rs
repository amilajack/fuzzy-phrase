#[allow(dead_code)]
mod tables;
use self::tables::{Script, script_table};

use std::collections::HashMap;

#[cfg(test)]
use regex::Regex;
#[cfg(test)]
use lazy_static;

// I want a few things here from the underlying tables:
// - a map from each script (enum type) to a list of relevant ranges
// - a map from each script (enum type) to its name (String)
// - conversely, a map from each script name to its enum type
// for the last two, because the underlying table doesn't come with strings,
// I'm going to egregiously abuse the Debug trait

lazy_static! {
    static ref SCRIPT_NAME_TO_ENUM: HashMap<String, Script> = {
        let mut hm = HashMap::new();
        for entry in script_table.iter() {
            let name = format!("{:?}", &entry.2);
            hm.entry(name).or_insert(entry.2.to_owned());
        }
        hm.insert("Unknown".to_string(), Script::Unknown);
        hm
    };
    static ref SCRIPT_ENUM_TO_NAME: HashMap<Script, String> = {
        let mut hm = HashMap::new();
        for (key, val) in SCRIPT_NAME_TO_ENUM.iter() {
            hm.insert(val.to_owned(), key.to_owned());
        }
        hm
    };
    static ref SCRIPT_ENUM_TO_RANGES: HashMap<Script, Vec<(char, char)>> = {
        let mut hm = HashMap::new();
        for entry in script_table.iter() {
            let v = hm.entry(entry.2.to_owned()).or_insert_with(|| Vec::new());
            v.push((entry.0, entry.1));
        }
        hm
    };
}

pub fn get_pattern_for_scripts(scripts: &[Script]) -> String {
    let mut ranges: Vec<(char, char)> = Vec::new();
    for script in scripts {
        ranges.extend_from_slice(SCRIPT_ENUM_TO_RANGES.get(script).unwrap());
    }
    ranges.sort();

    let ranges = if ranges.len() < 2 {
        ranges
    } else {
        let mut collapsed_ranges: Vec<(char, char)> = vec![ranges[0]];
        for i in 1..ranges.len() {
            if (collapsed_ranges.last().unwrap().1 as u32) + 1 == ranges[i].0 as u32 {
                // extend the top of the current last range
                collapsed_ranges.last_mut().unwrap().1 = ranges[i].1
            } else {
                // this range is not contiguous with the last one, so push it separately
                collapsed_ranges.push(ranges[i])
            }
        }
        collapsed_ranges
    };

    let mut out: String = "^[".to_string();
    for (start, end) in ranges {
        out.extend(start.escape_unicode());
        if start != end {
            out.push('-');
            out.extend(end.escape_unicode());
        }
    }
    out.push_str("]+$");
    out
}

pub fn get_script_name(script: &Script) -> String {
    SCRIPT_ENUM_TO_NAME.get(script).unwrap().clone()
}

pub fn get_script_by_name(name: &str) -> Option<Script> {
    match SCRIPT_NAME_TO_ENUM.get(name) {
        Some(s) => Some(s.to_owned()),
        None => None,
    }
}

#[test]
fn unicode_build() {
    lazy_static::initialize(&SCRIPT_NAME_TO_ENUM);
    lazy_static::initialize(&SCRIPT_ENUM_TO_NAME);
    lazy_static::initialize(&SCRIPT_ENUM_TO_RANGES);
}

#[test]
fn unicode_get_range() {
    let r_latin = Regex::new(&get_pattern_for_scripts(&vec![Script::Latin])).unwrap();
    let r_greek = Regex::new(&get_pattern_for_scripts(&vec![Script::Greek])).unwrap();
    let r_both = Regex::new(&get_pattern_for_scripts(&vec![Script::Latin, Script::Greek])).unwrap();

    let t_latin = "abcde";
    let t_greek = "καθέδρα";
    let t_both = "abcκαθ";

    assert!(r_latin.is_match(t_latin));
    assert!(!r_latin.is_match(t_greek));
    assert!(!r_latin.is_match(t_both));

    assert!(!r_greek.is_match(t_latin));
    assert!(r_greek.is_match(t_greek));
    assert!(!r_greek.is_match(t_both));

    assert!(r_both.is_match(t_latin));
    assert!(r_both.is_match(t_greek));
    assert!(r_both.is_match(t_both));
}