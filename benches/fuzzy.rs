#[cfg(test)] extern crate test_utils;

use criterion::{Criterion, Fun, Bencher};
use fuzzy_phrase::fuzzy::map::FuzzyMap;
use fuzzy_phrase::fuzzy::map::FuzzyMapBuilder;
use test_utils::*;
use std::rc::Rc;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::env;

pub fn benchmark(c: &mut Criterion) {
    extern crate tempfile;

    //share a list of words across benchmarks
    //and a built fuzzy map, so define a struct to contain them
    struct BenchData {
        words: Vec<String>,
        fuzzymap: FuzzyMap
    };

    let data_basename = match env::var("FUZZY_BENCH") {
        Ok(f) => {
            println!("file loc is {}", f);
            f
        },
        Err(..) => {
            println!("skipping fuzzy benchmarks");
            return
        },
    };
    let exact_data_loc = format!("{}.txt", data_basename);
    let f = File::open(exact_data_loc).expect("tried to open_file");
    let file_buf = BufReader::new(&f);

    let mut words: Vec<String> = vec![];
    for line in file_buf.lines() {
        let s: String = line.unwrap();
        words.push(s);
    }

    //build_to_iter expects a path to build the structure
    let dir = tempfile::tempdir().unwrap();
    let file_start = dir.path().join("fuzzy");
    // build the structure
    FuzzyMapBuilder::build_from_iter(&file_start, words.iter().map(|s| s.as_ref()) , 1);
    let map = unsafe { FuzzyMap::from_path(&file_start).unwrap() };

    // move the prebuilt data into a reference-counted struct
    let shared_data = Rc::new(BenchData { words: words, fuzzymap: map });
    // make a vector to fill with closures to bench-test
    let mut to_bench = Vec::new();

    //shared across bench runs
    let data = shared_data.clone();
    to_bench.push(Fun::new("exact_match", move |b: &mut Bencher, _i| {
        // we're benching on a list of words, but criterion needs to run for as long as it wants
        // to get a statistically significant sample, potentially for more iterations than we have
        // words, so we'll build all the benches around cycle iterators that go forever
        let mut cycle = shared_data.words.iter().cycle();

        //this is the part that is timed
        b.iter(|| {
            let _matches = shared_data.fuzzymap.lookup(cycle.next().unwrap(), 1, |id| &shared_data.words[id as usize]);
        });
    }));

    let d1_loc = format!("{}.txt", data_basename);
    let f = File::open(d1_loc).expect("tried to open_file");
    let file_buf = BufReader::new(&f);
    let mut typos: Vec<String> = vec![];
    for line in file_buf.lines() {
        let s: String = line.unwrap();
        //use damage_words in test_utils to create typos
        let damaged_word: String = damage_word(&s);
        typos.push(damaged_word);
    }

    to_bench.push(Fun::new("d1_not_exact_match", move |b: &mut Bencher, _i| {
        // we're benching on a list of words, but criterion needs to run for as long as it wants
        // to get a statistically significant sample, potentially for more iterations than we have
        // words, so we'll build all the benches around cycle iterators that go forever
        let mut cycle = typos.iter().cycle();

        //this is the part that is timed
        b.iter(|| {
            let _matches = data.fuzzymap.lookup(cycle.next().unwrap(), 1, |id| &typos[id as usize]);
        });
    }));
    c.bench_functions("fuzzy", to_bench, ());
}
