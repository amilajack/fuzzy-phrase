extern crate fst;
extern crate itertools;
extern crate memmap;

mod prefix;
pub use prefix::PrefixSet;
pub use prefix::PrefixSetBuilder;

mod fuzzy;
pub use fuzzy::FuzzySetBuilder;

