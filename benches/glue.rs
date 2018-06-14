use criterion::{Criterion, Fun, Bencher};
use reqwest;
use fuzzy_phrase::glue::*;
use test_utils::*;
use fst::raw::Output;
use std::rc::Rc;
use itertools::Itertools;
use tempfile;
use std::fs;
use rand;
use rand::Rng;
use std::io::{BufRead, BufReader};

pub fn benchmark(c: &mut Criterion) {
    // the things I'm going to set up once and share across benchmarks are a list of words
    // and a built prefix set, so define a struct to contain them
    struct BenchData {
        phrases: Vec<String>,
        set: FuzzyPhraseSet
    };

    let dir = tempfile::tempdir().unwrap();
    let phrases = {
        let test_data = ensure_data("phrase", "us", "en", "latn", true);

        let file = fs::File::open(test_data).unwrap();
        let file = BufReader::new(file);
        file.lines().filter_map(|l| match l.unwrap() {
            ref t if t.len() == 0 => None,
            t => Some(t),
        }).collect::<Vec<String>>()
    };
    let set: FuzzyPhraseSet = {
        let mut builder = FuzzyPhraseSetBuilder::new(&dir.path()).unwrap();
        for phrase in phrases.iter() {
            builder.insert_str(phrase).unwrap();
        }
        builder.finish().unwrap();

        FuzzyPhraseSet::from_path(&dir.path()).unwrap()
    };

    // move the prebuilt data into a reference-counted struct
    let shared_data = Rc::new(BenchData { phrases, set });
    // make a vector I'm going to fill with closures to bench-test
    let mut to_bench = Vec::new();

    // each closure gets its own copy of the prebuilt data, but the "copy" is cheap since it's an
    // RC -- this is just a new reference and an increment to the count
    //
    // the copy will the get moved into the closure, but the original will stick around to be
    // copied for the next one
    let data = shared_data.clone();
    to_bench.push(Fun::new("fuzzy_match", move |b: &mut Bencher, _i| {
        let mut damaged_phrases: Vec<String> = Vec::with_capacity(1000);
        let mut rng = rand::thread_rng();

        for _i in 0..1000 {
            let phrase = rng.choose(&data.phrases).unwrap();
            let damaged = get_damaged_phrase(phrase, |w| data.set.can_fuzzy_match(w));
            damaged_phrases.push(damaged);
        }

        let mut cycle = damaged_phrases.iter().cycle();
        // the closure based to b.iter is the thing that will actually be timed; everything before
        // that is untimed per-benchmark setup
        b.iter(|| data.set.fuzzy_match_str(cycle.next().unwrap(), 1, 1));
    }));

    // data is shadowed here for ease of copying and pasting, but this is a new clone
    // (again, same data, new reference, because it's an Rc)
    let data = shared_data.clone();
    to_bench.push(Fun::new("fuzzy_match_prefix", move |b: &mut Bencher, _i| {
        let mut damaged_phrases: Vec<String> = Vec::with_capacity(1000);
        let mut rng = rand::thread_rng();

        for _i in 0..1000 {
            let phrase = rng.choose(&data.phrases).unwrap();
            let damaged = get_damaged_prefix(phrase, |w| data.set.can_fuzzy_match(w));
            damaged_phrases.push(damaged);
        }

        let mut cycle = damaged_phrases.iter().cycle();
        // the closure based to b.iter is the thing that will actually be timed; everything before
        // that is untimed per-benchmark setup
        b.iter(|| data.set.fuzzy_match_prefix_str(cycle.next().unwrap(), 1, 1));
    }));

    // run the accumulated list of benchmarks
    c.bench_functions("glue", to_bench, ());
}