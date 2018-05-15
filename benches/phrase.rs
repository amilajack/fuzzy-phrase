use std::fs::File;
use std::io::{BufRead, BufReader};
use std::collections::HashMap;
use std::rc::Rc;
use itertools::Itertools;
use criterion::{Criterion, Fun, Bencher};
use reqwest;
use fst::raw::Output;
use fuzzy_phrase::{PhraseSet, PhraseSetBuilder};
use fuzzy_phrase::phrase::query::{QueryWord, QueryPhrase};

pub fn build_phrase_graph(file_loc: &str) -> (HashMap<String, u32>, Vec<Vec<u32>>, PhraseSet) {
    // fetch data and build the structures
    let mut autoinc = 0;
    let mut word_to_id: HashMap<String, u32> = HashMap::new();
    let f = File::open(file_loc).expect("tried to open_file");
    let mut file_buf = BufReader::new(&f);

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
       phrases.sort();
    }

    for phrase in phrases {
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
        // we're benching on a list of words, but criterion needs to run for as long as it wants
        // to get a statistically significant sample, potentially for more iterations than we have
        // words, so we'll build all the benches around cycle iterators that go forever
        let mut cycle = data.phrases.iter().cycle();
        // the closure based to b.iter is the thing that will actually be timed; everything before
        // that is untimed per-benchmark setup
        let next_query = || -> QueryPhrase {
            let word_ids = cycle.next().unwrap().as_slice();
            let query_words = word_ids.iter()
                .map(|w| QueryWord::Full{ id: *w, edit_distance: 0})
                .collect::<Vec<QueryWord>>();
            let mut sequence = vec![];
            for qw in query_words.iter() {
                sequence.push(qw);
            }
            let query_phrase = QueryPhrase::new(&sequence[..]).unwrap();
            data.phrase_set.contains(query_phrase);
        };

        b.iter(|| data.phrase_set.contains(get_next_query()));
    }));

    // run the accumulated list of benchmarks
    c.bench_functions("prefix", to_bench, ());
}
