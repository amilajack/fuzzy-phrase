// #[macro_use] extern crate failure as failure;

use failure::Error;
// use super::lib;

#[derive(Fail, Debug)]
// #[fail(display = "invalid toolchain name")]
enum PhraseSetError {
    #[fail(display = "invalid structure metadata: {}", name)]
    InvalidStructureMetadata {
        name: String,
    },
    #[fail(display = "unknown script: {}", script)]
    UnknownScript {
        script: String,
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
