extern crate reqwest;
extern crate libflate;
extern crate rand;
extern crate itertools;

use std::path::{Path, PathBuf};
use rand::Rng;
use std::io::{self, BufRead};
use std::fs;
use std::iter;
use libflate::gzip::Decoder;

static TMP: &'static str = "/tmp/fuzzy_phrase";

pub fn ensure_data(data_type: &str, country: &str, language: &str, script: &str, sample: bool) -> PathBuf {
    fs::create_dir_all(TMP).unwrap();

    let file_name = format!("{}_{}_{}{}.txt", country, language, script, if sample { "_sample" } else { "" });
    let path = Path::new(TMP).join(&file_name);

    if path.exists() {
        return path;
    }

    let url = format!("https://s3.amazonaws.com/mapbox/playground/boblannon/fuzzy-phrase/bench/{}/{}.gz", data_type, file_name);
    let req = reqwest::get(&url).unwrap();
    let mut decoder = Decoder::new(req).unwrap();

    let mut wtr = io::BufWriter::new(fs::File::create(&path).unwrap());

    io::copy(&mut decoder, &mut wtr).unwrap();
    path
}

pub fn get_data(data_type: &str, country: &str, language: &str, script: &str, sample: bool) -> Vec<String> {
    let test_data = ensure_data(data_type, country, language, script, sample);

    let file = fs::File::open(test_data).unwrap();
    let file = io::BufReader::new(file);
    file.lines().filter_map(|l| match l.unwrap() {
        ref t if t.len() == 0 => None,
        t => Some(t),
    }).collect::<Vec<String>>()
}

pub fn damage_word(word: &str) -> String {
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

pub fn random_trunc(word: &str) -> String {
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

pub fn get_damaged_phrase<F>(phrase: &str, can_damage: F) -> String where
    F: Fn(&str) -> bool {
    let mut rng = rand::thread_rng();

    let words = phrase.split(' ').collect::<Vec<_>>();

    let eligible_idxs = &words.iter().enumerate().filter_map(|(i, w)| if can_damage(w) { Some(i) } else { None }).collect::<Vec<_>>();

    let (damage_idx, damaged_word) = if eligible_idxs.len() > 0 {
        let i = rng.choose(eligible_idxs).unwrap();
        (*i, damage_word(words[*i]))
    } else {
        (words.len() + 1, "".to_string())
    };

    let new_words = words.iter().enumerate().map(|(i, w)| if damage_idx == i { damaged_word.as_str() } else { *w }).collect::<Vec<&str>>();
    itertools::join(new_words, " ")
}

pub fn get_damaged_prefix<F>(phrase: &str, can_damage: F) -> String where
    F: Fn(&str) -> bool {
    let mut rng = rand::thread_rng();

    let words = phrase.split(' ').collect::<Vec<_>>();
    let trunc_idx = rng.gen_range(0, words.len());
    let trunc_word = random_trunc(&words[trunc_idx]);

    let eligible_idxs = &words.iter().enumerate().filter_map(|(i, w)| if i < trunc_idx && can_damage(w) { Some(i) } else { None }).collect::<Vec<_>>();

    let (damage_idx, damaged_word) = if eligible_idxs.len() > 0 {
        // damage some word before the truncation word
        let i = rng.choose(eligible_idxs).unwrap();
        (*i, damage_word(words[*i]))
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

pub fn get_garbage_phrase(phrase_len_range: (usize, usize), word_len_range: (usize, usize)) -> String {
    let mut rng = rand::thread_rng();
    let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

    let phrase_len: usize = rng.gen_range(phrase_len_range.0, phrase_len_range.1);
    let mut phrase: Vec<String> = Vec::with_capacity(phrase_len);
    for _j in 0..phrase_len {
        let word_len: usize = rng.gen_range(word_len_range.0, word_len_range.1);
        phrase.push(iter::repeat(()).map(|()| {
            *rng.choose(letters.as_bytes()).unwrap() as char
        }).take(word_len).collect());
    }
    itertools::join(phrase, " ")
}