pub mod util;

use std::fs::File;
use std::io;
use fst;
use fst::{Streamer, IntoStreamer};

use self::util::phrase_to_key;

pub struct PhraseSet(fst::Set);

impl PhraseSet {

    /// Test membership of a single phrase
    pub fn contains(&self, phrase: &[u64]) -> bool {
        let key = phrase_to_key(&phrase);
        self.0.contains(key)
    }

    /// Create from a raw byte sequence, which must be written by `PhraseSetBuilder`.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, fst::Error> {
        fst::Set::from_bytes(bytes).map(PhraseSet)
    }

}

impl<'s, 'a> IntoStreamer<'a> for &'s PhraseSet {
    type Item = &'a [u8];
    type Into = fst::set::Stream<'s>;

    fn into_stream(self) -> Self::Into {
        self.0.stream()
    }
}

pub struct PhraseSetBuilder<W>(fst::SetBuilder<W>);

impl PhraseSetBuilder<Vec<u8>> {
    pub fn memory() -> Self {
        // PhraseSetBuilder(fst::SetBuilder(fst::raw::Builder::memory()))
        PhraseSetBuilder(fst::SetBuilder::memory())
    }

}

impl<W: io::Write> PhraseSetBuilder<W> {

    /// Insert a phrase, specified as an array of word identifiers.
    pub fn insert(&mut self, phrase: &[u64]) -> Result<(), fst::Error> {
        let key = phrase_to_key(phrase);
        self.0.insert(key)
    }

    pub fn into_inner(self) -> Result<W, fst::Error> {
        self.0.into_inner()
    }

}


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

        let phrase_set = PhraseSet::from_bytes(bytes).unwrap();

        let mut keys = vec![];
        let mut stream = phrase_set.into_stream();
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

