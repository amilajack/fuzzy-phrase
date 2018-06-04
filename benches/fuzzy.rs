use criterion::{Criterion, Fun, Bencher};
use reqwest;
use fuzzy_phrase::fuzzy::map::FuzzyMap;
use fuzzy_phrase::fuzzy::map::FuzzyMapBuilder;
use fst::raw::Output;
use std::rc::Rc;
use itertools::Itertools;

pub fn benchmark(c: &mut Criterion) {
    extern crate tempfile;
    struct BenchData {
        words: Vec<String>,
        fuzzy_map: FuzzyMap
    };

    // fetch data and build the structures
    let wordlist = reqwest::get("https://raw.githubusercontent.com/BurntSushi/fst/master/data/words-10000")
            .expect("tried to download data")
            .text().expect("tried to decode the data");
    let words = wordlist.trim().split("\n").map(|w| w.to_owned()).collect::<Vec<String>>();

    let dir = tempfile::tempdir().unwrap();
    let file_start = dir.path().join("fuzzy");
    FuzzyMapBuilder::build_from_iter(&file_start, words.iter().cloned(), 2).unwrap();
    let map = unsafe { FuzzyMap::from_path(&file_start).unwrap() };
    let shared_data = Rc::new(BenchData { words, map });
    let mut to_bench = Vec::new();

    let data = shared_data.clone();

    to_bench.push(Fun::new("exact_contains", move |b: &mut Bencher, _i| {
        let mut cycle = data.words.iter().cycle();
        b.iter(|| data.map.lookup(cycle.next().unwrap()));
    }));
    c.bench_functions("prefix", to_bench, ());
}
