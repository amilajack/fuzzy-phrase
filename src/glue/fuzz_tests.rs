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
    for _i in 0..50 {
        let phrase = rng.choose(&PHRASES).unwrap();
        let damaged = get_damaged_phrase(phrase);
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
    for _i in 0..50 {
        let phrase = rng.choose(&PHRASES).unwrap();
        let damaged = get_damaged_prefix(phrase);
        let results = SET.fuzzy_match_prefix_str(&damaged.as_str(), 1, 1);

        assert!(results.is_ok());
        if let Ok(res) = results {
            assert!(res.iter().filter(|result| phrase.starts_with(itertools::join(&result.phrase, " ").as_str())).count() > 0);
        }
    }
}