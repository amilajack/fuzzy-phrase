#[cfg(test)] extern crate tempfile;
#[cfg(test)] extern crate rand;
#[cfg(test)] extern crate lazy_static;
#[cfg(test)] extern crate test_utils;

use super::*;
use glue::fuzz_tests::test_utils::*;
use glue::fuzz_tests::rand::Rng;
use std::io::Read;
use std::fs;
use itertools;

lazy_static! {
    static ref DIR: tempfile::TempDir = tempfile::tempdir().unwrap();
    static ref DATA: String = {
        let test_data = ensure_data("phrase", "us", "en", "latn", true);

        let mut file = fs::File::open(test_data).unwrap();
        let mut data = String::new();
        file.read_to_string(&mut data).unwrap();
        data
    };
    static ref PHRASES: Vec<&'static str> = {
        DATA.trim().split("\n").collect::<Vec<&str>>()
    };
    static ref SET: FuzzyPhraseSet = {
        let mut builder = FuzzyPhraseSetBuilder::new(&DIR.path()).unwrap();
        for phrase in PHRASES.iter() {
            builder.insert_str(phrase).unwrap();
        }
        builder.finish().unwrap();

        FuzzyPhraseSet::from_path(&DIR.path()).unwrap()
    };
}

#[test]
#[ignore]
fn glue_fuzztest_build() {
    lazy_static::initialize(&SET);
}

#[test]
#[ignore]
fn glue_fuzztest_match() {
    let mut rng = rand::thread_rng();
    for _i in 0..500 {
        let phrase = rng.choose(&PHRASES).unwrap();
        let damaged = get_damaged_phrase(phrase, |w| SET.can_fuzzy_match(w));
        let results = SET.fuzzy_match_str(&damaged.as_str(), 1, 1);

        assert!(results.is_ok());
        if let Ok(res) = results {
            assert!(res.iter().filter(|result| itertools::join(&result.phrase, " ").as_str() == *phrase).count() > 0);
        }
    }
}

#[test]
#[ignore]
fn glue_fuzztest_match_prefix() {
    let mut rng = rand::thread_rng();
    for _i in 0..500 {
        let phrase = rng.choose(&PHRASES).unwrap();
        let damaged = get_damaged_prefix(phrase, |w| SET.can_fuzzy_match(w));
        let results = SET.fuzzy_match_prefix_str(&damaged.as_str(), 1, 1);

        assert!(results.is_ok());
        if let Ok(res) = results {
            assert!(res.iter().filter(|result| phrase.starts_with(itertools::join(&result.phrase, " ").as_str())).count() > 0);
        }
    }
}

#[test]
#[ignore]
fn fuzzy_match_windowed_multi_equivalent_test() {
    let cities: Vec<&str> = include_str!("../../benches/data/phrase_test_cities.txt").trim().split("\n").collect();
    let states: Vec<&str> = include_str!("../../benches/data/phrase_test_states.txt").trim().split("\n").collect();
    let mut rng = rand::thread_rng();
    let mut augmented_phrases: Vec<String> = Vec::with_capacity(1000);
    for _i in 0..1000 {
        let phrase = rng.choose(&PHRASES).unwrap();
        let damaged = get_damaged_phrase(phrase, |w| SET.can_fuzzy_match(w));
        let zip: u32 = rng.gen_range(10000, 99999);
        let augmented = format!(
            "{addr} {city} {state} {zip}",
            addr = damaged,
            city = rng.choose(&cities).unwrap(),
            state = rng.choose(&states).unwrap(),
            zip = zip
        );
        augmented_phrases.push(augmented);
    }

    for phrase in augmented_phrases.iter() {
        let tokens: Vec<_> = phrase.split(" ").collect();
        let mut variants: Vec<(Vec<&str>, bool)> = Vec::new();
        let windowed_match_result = SET.fuzzy_match_windows(tokens.as_slice(), 1, 1, false).unwrap();
        for start in 0..tokens.len() {
            for end in start..tokens.len() {
                variants.push((tokens[start..(end + 1)].to_vec(), false));
            }
        }
        let windowed_match_multi_result = SET.fuzzy_match_multi(variants.as_slice(), 1, 1).unwrap();
        //check if windowed match and multi windowed match give the same results
        assert!(windowed_match_result.iter().any(|x| {
            windowed_match_multi_result.iter().any(|y| {
                y.iter().any(|z| {
                    x == z
                })
            })
        }));
    }
}
