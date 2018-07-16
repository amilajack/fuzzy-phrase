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
fn glue_fuzztest_windowed_multi_equivalent() {
    let cities: Vec<&str> = include_str!("../../benches/data/phrase_test_cities.txt").trim().split("\n").collect();
    let states: Vec<&str> = include_str!("../../benches/data/phrase_test_states.txt").trim().split("\n").collect();
    let mut rng = rand::thread_rng();
    let mut augmented_phrases: Vec<String> = Vec::with_capacity(1000);
    for _i in 0..1000 {
        let phrase = rng.choose(&PHRASES).unwrap();
        let damaged = get_damaged_phrase(phrase, |w| SET.can_fuzzy_match(w));
        let zip: u32 = rng.gen_range(10000, 99999);

        // make a string with the components in random order
        let mut augmented_vec = vec![
            damaged,
            rng.choose(&cities).unwrap().to_string(),
            rng.choose(&states).unwrap().to_string(),
            zip.to_string()
        ];
        rng.shuffle(augmented_vec.as_mut_slice());
        let augmented = augmented_vec.join(" ");
        augmented_phrases.push(augmented);
    }

    for phrase in augmented_phrases.iter() {
        let tokens: Vec<_> = phrase.split(" ").collect();
        let mut variants: Vec<(Vec<&str>, bool)> = Vec::new();
        let mut variant_starts: Vec<usize> = Vec::new();
        for start in 0..tokens.len() {
            for end in start..tokens.len() {
                variants.push((tokens[start..(end + 1)].to_vec(), false));
                variant_starts.push(start);
            }
        }
        let individual_match_result = variants.iter().map(|v| SET.fuzzy_match(v.0.as_slice(), 1, 1).unwrap()).collect::<Vec<_>>();
        let multi_match_result = SET.fuzzy_match_multi(variants.as_slice(), 1, 1).unwrap();

        // check if the multi match results and the one-by-one match results are identical
        assert_eq!(individual_match_result, multi_match_result);

        // to make sure the windowed match and multi windowed match give the same results, we need
        // to reformat the multi-match results to look like windowed match results based on the
        // start position and length of each variant
        let mut windowed_match_result = SET.fuzzy_match_windows(tokens.as_slice(), 1, 1, false).unwrap();
        let mut emulated_windowed_match_result: Vec<FuzzyWindowResult> = Vec::new();
        for i in 0..multi_match_result.len() {
            for result in &multi_match_result[i] {
                emulated_windowed_match_result.push(FuzzyWindowResult {
                    phrase: result.phrase.clone(),
                    edit_distance: result.edit_distance,
                    start_position: variant_starts[i],
                    ends_in_prefix: false
                });
            }
        }

        windowed_match_result.sort();
        emulated_windowed_match_result.sort();

        assert_eq!(windowed_match_result, emulated_windowed_match_result);
    }
}

#[test]
#[ignore]
fn glue_fuzztest_windowed_multi_equivalent_prefix() {
    let cities: Vec<&str> = include_str!("../../benches/data/phrase_test_cities.txt").trim().split("\n").collect();
    let states: Vec<&str> = include_str!("../../benches/data/phrase_test_states.txt").trim().split("\n").collect();
    let mut rng = rand::thread_rng();
    let mut augmented_phrases: Vec<String> = Vec::with_capacity(1000);
    for _i in 0..100 {
        let phrase = rng.choose(&PHRASES).unwrap();
        let damaged = get_damaged_phrase(phrase, |w| SET.can_fuzzy_match(w));
        let zip: u32 = rng.gen_range(10000, 99999);

        // make a string with the components in random order
        let mut augmented_vec = vec![
            damaged,
            rng.choose(&cities).unwrap().to_string(),
            rng.choose(&states).unwrap().to_string(),
            zip.to_string()
        ];
        rng.shuffle(augmented_vec.as_mut_slice());
        let augmented = augmented_vec.join(" ");
        augmented_phrases.push(random_trunc(&augmented));
    }

    for phrase in augmented_phrases.iter() {
        let tokens: Vec<_> = phrase.split(" ").collect();
        let mut variants: Vec<(Vec<&str>, bool)> = Vec::new();
        let mut variant_starts: Vec<usize> = Vec::new();
        let mut variant_eip: Vec<bool> = Vec::new();
        for start in 0..tokens.len() {
            for end in start..tokens.len() {
                let ends_in_prefix = end + 1 == tokens.len();
                variants.push((tokens[start..(end + 1)].to_vec(), ends_in_prefix));
                variant_starts.push(start);
                variant_eip.push(ends_in_prefix);
            }
        }
        let individual_match_result = variants.iter().map(|v| if v.1 {
            SET.fuzzy_match_prefix(v.0.as_slice(), 1, 1).unwrap()
        } else {
            SET.fuzzy_match(v.0.as_slice(), 1, 1).unwrap()
        }).collect::<Vec<_>>();
        let multi_match_result = SET.fuzzy_match_multi(variants.as_slice(), 1, 1).unwrap();

        // check if the multi match results and the one-by-one match results are identical
        assert_eq!(individual_match_result, multi_match_result);

        // to make sure the windowed match and multi windowed match give the same results, we need
        // to reformat the multi-match results to look like windowed match results based on the
        // start position and length of each variant
        let mut windowed_match_result = SET.fuzzy_match_windows(tokens.as_slice(), 1, 1, true).unwrap();
        let mut emulated_windowed_match_result: Vec<FuzzyWindowResult> = Vec::new();
        for i in 0..multi_match_result.len() {
            for result in &multi_match_result[i] {
                emulated_windowed_match_result.push(FuzzyWindowResult {
                    phrase: result.phrase.clone(),
                    edit_distance: result.edit_distance,
                    start_position: variant_starts[i],
                    ends_in_prefix: variant_eip[i]
                });
            }
        }

        windowed_match_result.sort();
        emulated_windowed_match_result.sort();

        assert_eq!(windowed_match_result, emulated_windowed_match_result);
    }
}