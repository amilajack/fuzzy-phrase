extern crate fst;
extern crate byteorder;

#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

mod prefix;
pub use prefix::PrefixSet;
pub use prefix::PrefixSetBuilder;

pub mod phrase;

pub mod glue;
