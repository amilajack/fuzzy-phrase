#[macro_use]
extern crate criterion;
extern crate fuzzy_phrase;
extern crate fst;
extern crate reqwest;
extern crate itertools;

use criterion::Criterion;

mod prefix;
mod phrase;

// criterion_group!(benches, prefix::benchmark);
criterion_group!(benches, phrase::benchmark);
criterion_main!(benches);
