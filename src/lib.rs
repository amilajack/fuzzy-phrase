extern crate fst;
extern crate itertools;
extern crate memmap;
extern crate serde;
extern crate strsim;
extern crate byteorder;
#[macro_use]
extern crate serde_derive;
extern crate rmp_serde as rmps;

mod prefix;
pub use prefix::PrefixSet;
pub use prefix::PrefixSetBuilder;

mod fuzzy;
pub use fuzzy::FuzzyMap;
pub use fuzzy::FuzzyMapBuilder;
pub mod phrase;
