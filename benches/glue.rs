use criterion::{Criterion, Fun, Bencher};
use fuzzy_phrase::glue::*;
use test_utils::*;
use std::rc::Rc;
use itertools;
use tempfile;
use rand;
use rand::Rng;

pub fn benchmark(c: &mut Criterion) {
    // the things I'm going to set up once and share across benchmarks are a list of words
    // and a built prefix set, so define a struct to contain them
    struct BenchData {
        phrases: Vec<String>,
        set: FuzzyPhraseSet
    };

    let dir = tempfile::tempdir().unwrap();
    let phrases = get_data("phrase", "us", "en", "latn", true);
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

    let data = shared_data.clone();
    to_bench.push(Fun::new("fuzzy_match_failed_lt_latn", move |b: &mut Bencher, _i| {
        let lt_data = get_data("phrase", "lt", "lt", "latn", true);
        let mut cycle = lt_data.iter().cycle();

        b.iter(|| data.set.fuzzy_match_str(cycle.next().unwrap(), 1, 1));
    }));

    let data = shared_data.clone();
    to_bench.push(Fun::new("fuzzy_match_failed_ua_cyrl", move |b: &mut Bencher, _i| {
        let ua_data = get_data("phrase", "ua", "uk", "cyrl", true);
        let mut cycle = ua_data.iter().cycle();

        b.iter(|| data.set.fuzzy_match_str(cycle.next().unwrap(), 1, 1));
    }));

    let data = shared_data.clone();
    to_bench.push(Fun::new("fuzzy_match_failed_short_garbage", move |b: &mut Bencher, _i| {
        let mut garbage_phrases: Vec<String> = Vec::with_capacity(1000);
        for _i in 0..1000 {
            garbage_phrases.push(get_garbage_phrase((2, 10), (2, 10)));
        }

        let mut cycle = garbage_phrases.iter().cycle();

        b.iter(|| data.set.fuzzy_match_str(cycle.next().unwrap(), 1, 1));
    }));

    let data = shared_data.clone();
    to_bench.push(Fun::new("fuzzy_match_failed_long_garbage", move |b: &mut Bencher, _i| {
        let mut garbage_phrases: Vec<String> = Vec::with_capacity(1000);
        for _i in 0..1000 {
            garbage_phrases.push(get_garbage_phrase((8, 12), (100, 200)));
        }

        let mut cycle = garbage_phrases.iter().cycle();

        b.iter(|| data.set.fuzzy_match_str(cycle.next().unwrap(), 1, 1));
    }));

    // run the accumulated list of benchmarks
    c.bench_functions("glue", to_bench, ());
}