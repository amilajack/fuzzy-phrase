extern crate fst;
extern crate itertools;
extern crate memmap;
extern crate strsim;
extern crate byteorder;

mod prefix;
pub use prefix::PrefixSet;
pub use prefix::PrefixSetBuilder;

mod fuzzy;
pub use fuzzy::FuzzyMap;
pub use fuzzy::FuzzyMapBuilder;
pub mod phrase;
