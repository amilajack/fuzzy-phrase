// #[macro_use]extern crate failure as failure;

use failure::{Error, Fail};
// use super::lib;

#[derive(Debug, Fail)]
enum PhraseSetError {
    #[fail(display = "invalid toolchain name: {}", name)]
    InvalidStructureMetadata {
        name: String,
    },
    #[fail(display = "unknown toolchain version: {}", version)]
    UnknownScript {
        name: String,
    }


}

// #[derive(Debug, Clone, Fail)]
// pub struct PhraseSetError {
//     details: String
// }
//
// impl PhraseSetError {
//     pub fn new(msg: &str) -> PhraseSetError {
//         PhraseSetError{details: msg.to_string()}
//     }
// }
//
// impl fmt::Display for PhraseSetError {
//
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "{}", self.details)
//     }
// }
