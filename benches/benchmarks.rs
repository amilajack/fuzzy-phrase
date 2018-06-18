#[macro_use]
extern crate criterion;
extern crate fuzzy_phrase;
extern crate fst;
extern crate reqwest;
extern crate itertools;
extern crate rand;
extern crate test_utils;
extern crate tempfile;

use criterion::Criterion;

mod prefix;
mod phrase;
mod fuzzy;
mod glue;

criterion_group!{
    name = benches;
    config = Criterion::default();

    targets = prefix::benchmark, phrase::benchmark, glue::benchmark, fuzzy::benchmark
}
criterion_main!(benches);
