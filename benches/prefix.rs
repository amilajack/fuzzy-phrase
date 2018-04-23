use criterion::{Criterion, Fun, Bencher};
use reqwest;
use fuzzy_phrase::PrefixSet;
use fst::raw::Output;
use std::rc::Rc;
use itertools::Itertools;

pub fn benchmark(c: &mut Criterion) {
    // the things I'm going to set up once and share across benchmarks are a list of words
    // and a built prefix set, so define a struct to contain them
    struct BenchData {
        words: Vec<String>,
        prefix_set: PrefixSet
    };

    // fetch data and build the structures
    let wordlist = reqwest::get("https://raw.githubusercontent.com/BurntSushi/fst/master/data/words-10000")
            .expect("tried to download data")
            .text().expect("tried to decode the data");
    let words = wordlist.trim().split("\n").map(|w| w.to_owned()).collect::<Vec<String>>();
    let prefix_set = PrefixSet::from_iter(words.iter()).expect("tried to create prefix set");

    // move the prebuilt data into a reference-counted struct
    let shared_data = Rc::new(BenchData { words, prefix_set });
    // make a vector I'm going to fill with closures to bench-test
    let mut to_bench = Vec::new();

    // each closure gets its own copy of the prebuilt data, but the "copy" is cheap since it's an
    // RC -- this is just a new reference and an increment to the count
    //
    // the copy will the get moved into the closure, but the original will stick around to be
    // copied for the next one
    let data = shared_data.clone();
    to_bench.push(Fun::new("exact_contains", move |b: &mut Bencher, _i| {
        // we're benching on a list of words, but criterion needs to run for as long as it wants
        // to get a statistically significant sample, potentially for more iterations than we have
        // words, so we'll build all the benches around cycle iterators that go forever
        let mut cycle = data.words.iter().cycle();
        // the closure based to b.iter is the thing that will actually be timed; everything before
        // that is untimed per-benchmark setup
        b.iter(|| data.prefix_set.contains(cycle.next().unwrap()));
    }));

    // data is shadowed here for ease of copying and pasting, but this is a new clone
    // (again, same data, new reference, because it's an Rc)
    let data = shared_data.clone();
    to_bench.push(Fun::new("exact_contains_prefix", move |b: &mut Bencher, _i| {
        let mut cycle = data.words.iter().cycle();
        b.iter(|| data.prefix_set.contains_prefix(cycle.next().unwrap()));
    }));

    let data = shared_data.clone();
    to_bench.push(Fun::new("exact_get", move |b: &mut Bencher, _i| {
        let mut cycle = data.words.iter().cycle();
        b.iter(|| data.prefix_set.get(cycle.next().unwrap()));
    }));

    let data = shared_data.clone();
    to_bench.push(Fun::new("exact_get_prefix_range", move |b: &mut Bencher, _i| {
        let mut cycle = data.words.iter().cycle();
        b.iter(|| data.prefix_set.get_prefix_range(cycle.next().unwrap()));
    }));

    let data = shared_data.clone();
    to_bench.push(Fun::new("prefix_contains_prefix", move |b: &mut Bencher, _i| {
        // this benchmark needs a modified copy of the wordlist (specifically, a list of every
        // word without its last letter), so build that beforehand and collect it to make sure
        // it doesn't count as part of the time
        let prefixes = data.words.iter().map(|w| {
            let char_count = w.chars().count();
            w.chars().take(char_count - 1).collect()
        }).collect::<Vec<String>>();
        let mut cycle = prefixes.iter().cycle();
        b.iter(|| data.prefix_set.contains_prefix(cycle.next().unwrap()));
    }));

    let data = shared_data.clone();
    to_bench.push(Fun::new("short_get_prefix_range", move |b: &mut Bencher, _i| {
        let prefixes = data.words.iter().map(|w| {
            let char_count = w.chars().count();
            w.chars().take(if char_count < 2 { char_count } else { 2 }).collect()
        }).dedup().collect::<Vec<String>>();
        let mut cycle = prefixes.iter().cycle();
        b.iter(|| data.prefix_set.get_prefix_range(cycle.next().unwrap()));
    }));

    let data = shared_data.clone();
    to_bench.push(Fun::new("get_by_id", move |b: &mut Bencher, _i| {
        // this one is a cycle of numbers instead of strings
        let counts = 0..data.words.len();
        let mut cycle = counts.cycle();
        b.iter(|| data.prefix_set.get_by_id(Output::new(cycle.next().unwrap() as u64)));
    }));

    // run the accumulated list of benchmarks
    c.bench_functions("prefix", to_bench, ());
}