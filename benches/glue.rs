use criterion::{Criterion, Fun, Bencher};
use fuzzy_phrase::glue::*;
use test_utils::*;
use std::rc::Rc;
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
    to_bench.push(Fun::new("fuzzy_match_full_success", move |b: &mut Bencher, _i| {
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
    to_bench.push(Fun::new("fuzzy_match_prefix_success", move |b: &mut Bencher, _i| {
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
    to_bench.push(Fun::new("fuzzy_match_lt_latn_failure", move |b: &mut Bencher, _i| {
        let lt_data = get_data("phrase", "lt", "lt", "latn", true);
        let mut cycle = lt_data.iter().cycle();

        b.iter(|| data.set.fuzzy_match_str(cycle.next().unwrap(), 1, 1));
    }));

    let data = shared_data.clone();
    to_bench.push(Fun::new("fuzzy_match_ua_cyrl_failure", move |b: &mut Bencher, _i| {
        let ua_data = get_data("phrase", "ua", "uk", "cyrl", true);
        let mut cycle = ua_data.iter().cycle();

        b.iter(|| data.set.fuzzy_match_str(cycle.next().unwrap(), 1, 1));
    }));

    let data = shared_data.clone();
    to_bench.push(Fun::new("fuzzy_match_short_garbage_failure", move |b: &mut Bencher, _i| {
        let mut garbage_phrases: Vec<String> = Vec::with_capacity(1000);
        for _i in 0..1000 {
            garbage_phrases.push(get_garbage_phrase((2, 10), (2, 10)));
        }

        let mut cycle = garbage_phrases.iter().cycle();

        b.iter(|| data.set.fuzzy_match_str(cycle.next().unwrap(), 1, 1));
    }));

    let data = shared_data.clone();
    to_bench.push(Fun::new("fuzzy_match_long_garbage_failure", move |b: &mut Bencher, _i| {
        let mut garbage_phrases: Vec<String> = Vec::with_capacity(1000);
        for _i in 0..1000 {
            garbage_phrases.push(get_garbage_phrase((8, 12), (100, 200)));
        }

        let mut cycle = garbage_phrases.iter().cycle();

        b.iter(|| data.set.fuzzy_match_str(cycle.next().unwrap(), 1, 1));
    }));

    // these next few will construct some phrases that have fake additional cities/states/zips
    // to test windowing and multi-lookup tests on
    let data = shared_data.clone();
    let cities: Vec<&str> = include_str!("./data/phrase_test_cities.txt").trim().split("\n").collect();
    let states: Vec<&str> = include_str!("./data/phrase_test_states.txt").trim().split("\n").collect();
    let mut rng = rand::thread_rng();
    let mut augmented_phrases: Vec<String> = Vec::with_capacity(1000);
    for _i in 0..1000 {
        let phrase = rng.choose(&data.phrases).unwrap();
        let damaged = get_damaged_phrase(phrase, |w| data.set.can_fuzzy_match(w));
        let zip: u32 = rng.gen_range(10000, 99999);
        let augmented = format!(
            "{addr} {city} {state} {zip}",
            addr = damaged,
            city = rng.choose(&cities).unwrap(),
            state = rng.choose(&states).unwrap(),
            zip = zip
        );
        augmented_phrases.push(augmented);
    }
    let augmented_phrases = Rc::new(augmented_phrases);

    let data = shared_data.clone();
    let sample = augmented_phrases.clone();
    to_bench.push(Fun::new("fuzzy_match_complex_manual", move |b: &mut Bencher, _i| {
        let mut cycle = sample.iter().cycle();

        b.iter(|| {
            let tokens: Vec<_> = cycle.next().unwrap().split(" ").collect();
            for start in 0..tokens.len() {
                for end in start..tokens.len() {
                    data.set.fuzzy_match(&tokens[start..(end + 1)], 1, 1).unwrap();
                }
            }
        });
    }));

    let data = shared_data.clone();
    let sample = augmented_phrases.clone();
    to_bench.push(Fun::new("fuzzy_match_complex_multi", move |b: &mut Bencher, _i| {
        let mut cycle = sample.iter().cycle();

        b.iter(|| {
            let tokens: Vec<_> = cycle.next().unwrap().split(" ").collect();
            let mut variants: Vec<(Vec<&str>, bool)> = Vec::new();
            for start in 0..tokens.len() {
                for end in start..tokens.len() {
                    variants.push((tokens[start..(end + 1)].to_vec(), false));
                }
            }
            data.set.fuzzy_match_multi(variants.as_slice(), 1, 1).unwrap();
        });
    }));

    let data = shared_data.clone();
    let sample = augmented_phrases.clone();
    to_bench.push(Fun::new("fuzzy_match_complex_windows", move |b: &mut Bencher, _i| {
        let mut cycle = sample.iter().cycle();

        b.iter(|| {
            let tokens: Vec<_> = cycle.next().unwrap().split(" ").collect();
            data.set.fuzzy_match_windows(tokens.as_slice(), 1, 1, false).unwrap();
        });
    }));

    // run the accumulated list of benchmarks
    c.bench_functions("glue", to_bench, ());
}