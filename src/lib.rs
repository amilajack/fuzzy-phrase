extern crate fst;
extern crate byteorder;

mod prefix;
pub use prefix::PrefixSet;
pub use prefix::PrefixSetBuilder;

pub mod phrase;

pub use phrase::PhraseSet;
pub use phrase::PhraseSetBuilder;
pub use phrase::query::QueryPhrase;
pub use phrase::query::QueryWord;

