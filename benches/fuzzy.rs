use criterion::{Criterion, Fun, Bencher};
use reqwest;
use fuzzy_phrase::fuzzy::map::FuzzyMap;
use fuzzy_phrase::fuzzy::map::FuzzyMapBuilder;
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

    // fetch data and build the structures
    let wordlist = reqwest::get("https://raw.githubusercontent.com/BurntSushi/fst/master/data/words-10000")
            .expect("tried to download data")
            .text().expect("tried to decode the data");

    let words = wordlist.trim().split("\n").map(|w| w.to_owned()).collect::<Vec<String>>();

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

    let data_basename = match env::var("FUZZY_BENCH") {
        Ok(f) => {
            println!("file loc is {}", f);
            f
        },
        Err(..) => String::from("/tmp/fuzzy-phrase-bench/fuzzy/fuzzy_d1"),
    };
    let data_loc = format!("{}.txt", data_basename);
    // run benchmarks
    let f = File::open(data_loc).expect("tried to open_file");

    let file_buf = BufReader::new(&f);
    let mut typos: Vec<String> = vec![];
    for line in file_buf.lines() {
       let s: String = line.unwrap();
        typos.push(s);
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
