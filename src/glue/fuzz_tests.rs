#[cfg(test)] extern crate reqwest;
#[cfg(test)] extern crate libflate;
#[cfg(test)] extern crate tempfile;
#[cfg(test)] extern crate rand;
#[cfg(test)] extern crate lazy_static;

use super::*;
use glue::fuzz_tests::libflate::gzip::Decoder;
use glue::fuzz_tests::rand::Rng;
use std::io;
use std::io::Read;
use std::fs;
use itertools;

fn ensure_data() {
    if Path::new("/tmp/us_en_latn.txt").exists() {
        return;
    }

    let req = reqwest::get("https://s3.amazonaws.com/mapbox/playground/boblannon/fuzzy-phrase/bench/phrase/us_en_latn_sample.txt.gz").unwrap();
    let mut decoder = Decoder::new(req).unwrap();

    let mut wtr = io::BufWriter::new(fs::File::create("/tmp/us_en_latn_sample.txt").unwrap());

    io::copy(&mut decoder, &mut wtr).unwrap();
}

fn damage_word(word: &str) -> String {
    let letters = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'];
    enum EditType {
        Insert,
        Delete,
        Substitute,
        Transpose,
    }

    let mut rng = rand::thread_rng();

    let indices: Vec<_> = word.char_indices().collect();

    let operations = if word.len() > 1 {
        vec![EditType::Insert, EditType::Delete, EditType::Substitute, EditType::Transpose]
    } else {
        vec![EditType::Insert, EditType::Substitute]
    };

    match rng.choose(&operations).unwrap() {
        EditType::Insert => {
            // one more slots than there are letters
            let idx = rng.gen_range(0, indices.len() + 1);
            let pos = if idx == indices.len() {
                word.len()
            } else {
                indices[idx].0
            };
            let mut out = String::new();
            out.push_str(&word[..pos]);
            out.push(*rng.choose(&letters).unwrap());
            out.push_str(&word[pos..]);
            out
        },
        EditType::Delete => {
            // same number of slots as there are letters
            let idx = rng.gen_range(0, indices.len());
            let pos = indices[idx].0;
            let next_pos = if idx + 1 < indices.len() {
                indices[idx + 1].0
            } else {
                word.len()
            };

            let mut out = String::new();
            out.push_str(&word[..pos]);
            out.push_str(&word[next_pos..]);
            out
        },
        EditType::Substitute => {
            // same number of slots as there are letters
            let idx = rng.gen_range(0, indices.len());
            let pos = indices[idx].0;
            let next_pos = if idx + 1 < indices.len() {
                indices[idx + 1].0
            } else {
                word.len()
            };

            let mut out = String::new();
            out.push_str(&word[..pos]);
            out.push(*rng.choose(&letters).unwrap());
            out.push_str(&word[next_pos..]);
            out
        },
        EditType::Transpose => {
            // one fewer slots than there are letters -- implicates two letters
            let idx = rng.gen_range(0, indices.len() - 1);

            let first_char = indices[idx].0;
            let second_char = indices[idx + 1].0;
            let third_char = if idx + 2 < indices.len() {
                indices[idx + 2].0
            } else {
                word.len()
            };

            let mut out = String::new();
            // swap word[position] and s[position + 1]
            out.push_str(&word[..first_char]);
            out.push_str(&word[second_char..third_char]);
            out.push_str(&word[first_char..second_char]);
            out.push_str(&word[third_char..]);
            out
        },
    }
}

fn random_trunc(word: &str) -> String {
    let mut rng = rand::thread_rng();

    let indices: Vec<_> = word.char_indices().collect();
    let idx = rng.gen_range(1, indices.len() + 1);
    let pos = if idx < indices.len() {
        indices[idx].0
    } else {
        word.len()
    };
    word[..pos].to_owned()
}

fn get_damaged_phrase(phrase: &str) -> String {
    let mut rng = rand::thread_rng();

    let words = phrase.split(' ').collect::<Vec<_>>();
    let idx = rng.gen_range(0, words.len());
    let damaged = damage_word(words[idx]);
    let new_words = words.iter().enumerate().map(|(i, w)| if idx == i { damaged.as_str() } else { *w }).collect::<Vec<&str>>();
    itertools::join(new_words, " ")
}

fn get_damaged_prefix(phrase: &str) -> String {
    let mut rng = rand::thread_rng();

    let words = phrase.split(' ').collect::<Vec<_>>();
    let trunc_idx = rng.gen_range(0, words.len());
    let trunc_word = random_trunc(&words[trunc_idx]);

    let (damage_idx, damaged_word) = if trunc_idx > 0 {
        // damage some word before the truncation word
        let i = rng.gen_range(0, trunc_idx);
        (i, damage_word(words[i]))
    } else {
        // if we're truncating the first word, there aren't any to break, so set a high idx we
        // won't actually reach in the map
        (words.len() + 1, "".to_string())
    };

    let new_words = words.iter().enumerate().filter_map(|(i, w)| {
        match i {
            n if n == damage_idx => Some(damaged_word.as_str()),
            n if n == trunc_idx => Some(trunc_word.as_str()),
            n if n < trunc_idx => Some(*w),
            _ => None
        }
    }).collect::<Vec<&str>>();
    itertools::join(new_words, " ")
}

lazy_static! {
    static ref DIR: tempfile::TempDir = tempfile::tempdir().unwrap();
    static ref DATA: String = {
        ensure_data();

        let mut file = fs::File::open("/tmp/us_en_latn_sample.txt").unwrap();
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