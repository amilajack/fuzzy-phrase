use std::fs::File;
use std::io::{BufRead, BufReader};
use std::collections::HashMap;
use std::rc::Rc;
use criterion::{Criterion, Fun, Bencher};
use fuzzy_phrase::{PhraseSet, PhraseSetBuilder};
use fuzzy_phrase::phrase::query::{QueryWord, QueryPhrase};

pub fn build_phrase_graph(file_loc: &str) -> (HashMap<String, u32>, Vec<Vec<u32>>, PhraseSet) {
    // fetch data and build the structures
    let mut autoinc = 0;
    let mut word_to_id: HashMap<String, u32> = HashMap::new();
    let f = File::open(file_loc).expect("tried to open_file");
    let file_buf = BufReader::new(&f);

    let mut build = PhraseSetBuilder::memory();

    let mut phrases: Vec<Vec<u32>> = vec![];
    for line in file_buf.lines() {
       let s: String = line.unwrap();
       let mut word_ids: Vec<u32> = vec![];
       for word in s.as_str().split(" ").map(|w| w.to_lowercase()) {
           let word_id = word_to_id.entry(word.to_owned()).or_insert(autoinc);
           word_ids.push(*word_id);
           autoinc += 1;
       }
       phrases.push(word_ids);
    }

    phrases.sort();

    for phrase in phrases.iter() {
        build.insert(&phrase).unwrap();
    }

    let bytes = build.into_inner().unwrap();

    let phrase_set = PhraseSet::from_bytes(bytes).unwrap();
    return (word_to_id, phrases, phrase_set)
}



pub fn benchmark(c: &mut Criterion) {
    // the things I'm going to set up once and share across benchmarks are a list of words
    // and a built prefix set, so define a struct to contain them
    struct BenchData {
        word_to_id: HashMap<String, u32>,
        phrases: Vec<Vec<u32>>,
        phrase_set: PhraseSet
    };

    let (word_to_id, phrases, phrase_set) = build_phrase_graph("./benches/data/phrase_test.txt");

    // move the prebuilt data into a reference-counted struct
    let shared_data = Rc::new(BenchData { word_to_id, phrases, phrase_set });

    // make a vector I'm going to fill with closures to bench-test
    let mut to_bench = Vec::new();

    // each closure gets its own copy of the prebuilt data, but the "copy" is cheap since it's an
    // RC -- this is just a new reference and an increment to the count
    //
    // the copy will the get moved into the closure, but the original will stick around to be
    // copied for the next one
    let data = shared_data.clone();

    to_bench.push(Fun::new("exact_contains", move |b: &mut Bencher, _i| {
        // we're benching on a list of phrases, but criterion needs to run for as long as it wants
        // to get a statistically significant sample, potentially for more iterations than we have
        // words, so we'll build all the benches around cycle iterators that go forever
        let mut cycle = data.phrases.iter().cycle();

        // the closure based to b.iter is the thing that will actually be timed; everything before
        // that is untimed per-benchmark setup
        b.iter(|| {
            let query_ids = cycle.next().unwrap();
            let query_words = query_ids.iter()
                .map(|w| QueryWord::Full{ id: *w, edit_distance: 0})
                .collect::<Vec<QueryWord>>();
            let query_phrase = QueryPhrase::new(&query_words).unwrap();
            let result = data.phrase_set.contains(query_phrase).unwrap();
        });
    }));

    // data is shadowed here for ease of copying and pasting, but this is a new clone
    // (again, same data, new reference, because it's an Rc)
    let data = shared_data.clone();
    to_bench.push(Fun::new("exact_contains_prefix", move |b: &mut Bencher, _i| {
        let mut cycle = data.phrases.iter().cycle();
        b.iter(|| {
            let query_ids = cycle.next().unwrap();
            let query_words = query_ids.iter()
                .map(|w| QueryWord::Full{ id: *w, edit_distance: 0})
                .collect::<Vec<QueryWord>>();
            let query_phrase = QueryPhrase::new(&query_words).unwrap();
            let result = data.phrase_set.contains_prefix(query_phrase).unwrap();
        });
    }));

    // data is shadowed here for ease of copying and pasting, but this is a new clone
    // (again, same data, new reference, because it's an Rc)
    let data = shared_data.clone();
    to_bench.push(Fun::new("range_contains_prefix", move |b: &mut Bencher, _i| {
        let mut cycle = data.phrases.iter().cycle();
        b.iter(|| {
            let word_ids = cycle.next().unwrap();
            let fullword_ids = &word_ids[..word_ids.len()];
            let last_id = &word_ids[word_ids.len()-1];
            let last_id_range = (last_id - 2, last_id + 2);
            let mut query_words = fullword_ids.iter()
                .map(|w| QueryWord::Full{ id: *w, edit_distance: 0})
                .collect::<Vec<QueryWord>>();
            query_words.push(QueryWord::Prefix{ id_range: last_id_range});
            let query_phrase = QueryPhrase::new(&query_words).unwrap();
            let result = data.phrase_set.contains_prefix(query_phrase).unwrap();
        });
    }));

    // run the accumulated list of benchmarks
    c.bench_functions("phrase", to_bench, ());
}
