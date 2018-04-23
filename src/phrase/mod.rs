pub mod util;
pub mod set;

use std::fs::File;
use std::io;
use self::set::{PhraseSet, PhraseSetBuilder};
use fst::{Streamer, IntoStreamer};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_phrases() {
        let mut wtr = io::BufWriter::new(File::create("phrase-set.fst").unwrap());

        let mut build = PhraseSetBuilder::memory();
        build.insert(&[1u64, 61_528_u64, 561_528u64]).unwrap();
        build.insert(&[61_528_u64, 561_528u64, 1u64]).unwrap();
        build.insert(&[561_528u64, 1u64, 61_528_u64]).unwrap();
        let bytes = build.into_inner().unwrap();

        let phraseSet = PhraseSet::from_bytes(bytes).unwrap();

        let mut keys = vec![];
        let mut stream = phraseSet.into_stream();
        while let Some(key) = stream.next() {
            keys.push(key.to_vec());
        }
        let comp: Vec<Vec<u8>> = vec![vec![0u8, 0u8, 0u8]];
        assert_eq!(
            keys,
            vec![
                vec![
                    0u8, 0u8,   1u8,     // 1
                    0u8, 240u8, 88u8,    // 61_528
                    8u8, 145u8, 120u8    // 561_528
                ],
                vec![
                    0u8, 240u8, 88u8,    // 61_528
                    8u8, 145u8, 120u8,   // 561_528
                    0u8, 0u8,   1u8      // 1
                ],
                vec![
                    8u8, 145u8, 120u8,   // 561_528
                    0u8, 0u8,   1u8,     // 1
                    0u8, 240u8, 88u8     // 61_528
                ],
            ]
        );
    }
}

