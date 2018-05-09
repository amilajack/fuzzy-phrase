pub mod util;
pub mod query;

use std::io;
#[cfg(feature = "mmap")]
use std::path::Path;

use fst;
use fst::{IntoStreamer, Set, SetBuilder};

use self::util::word_ids_to_key;
use self::query::{QueryWord, QueryPhrase};

pub struct PhraseSet(Set);

/// PhraseSet is a lexicographically ordered set of phrases.
///
/// Phrases are sequences of words, where each word is represented as an integer. The integers
/// correspond to FuzzyMap values. Due to limitations in the fst library, however, the integers are
/// encoded as a series of 3 bytes.  For example, the three-word phrase "1## Main Street" will be
/// represented over 9 transitions, with one byte each.
///
/// | tokens  | integers  | three_bytes   |
/// |---------|-----------|---------------|
/// | 100     | 21        | [0,   0,  21] |
/// | main    | 457       | [0,   1, 201] |
/// | street  | 109821    | [1, 172, 253] |
///
impl PhraseSet {

    /// Test membership of a single phrase
    pub fn contains(&self, phrase: &[u64]) -> bool {
        let key = phrase_to_key(&phrase);
        self.0.contains(key)
    }

    /// Create from a raw byte sequence, which must be written by `PhraseSetBuilder`.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, fst::Error> {
        Set::from_bytes(bytes).map(PhraseSet)
    }

    #[cfg(feature = "mmap")]
    pub unsafe fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, fst::Error> {
        Set::from_path(path).map(PhraseSet)
    }


}

impl<'s, 'a> IntoStreamer<'a> for &'s PhraseSet {
    type Item = &'a [u8];
    type Into = fst::set::Stream<'s>;

    fn into_stream(self) -> Self::Into {
        self.0.stream()
    }
}

pub struct PhraseSetBuilder<W>(SetBuilder<W>);

impl PhraseSetBuilder<Vec<u8>> {
    pub fn memory() -> Self {
        PhraseSetBuilder(SetBuilder::memory())
    }

}

impl<W: io::Write> PhraseSetBuilder<W> {

    pub fn new(wtr: W) -> Result<PhraseSetBuilder<W>, fst::Error> {
        SetBuilder::new(wtr).map(PhraseSetBuilder)
    }

    /// Insert a phrase, specified as an array of word identifiers.
    pub fn insert(&mut self, phrase: &[u64]) -> Result<(), fst::Error> {
        let key = word_ids_to_key(phrase);
        self.0.insert(key)
    }

    pub fn into_inner(self) -> Result<W, fst::Error> {
        self.0.into_inner()
    }

    pub fn finish(self) -> Result<(), fst::Error> {
        self.0.finish()
    }

}


#[cfg(test)]
mod tests {
    use std::fs::File;
    use fst::Streamer;
    use super::*;

    #[test]
    fn insert_phrases_memory() {
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

    #[test]
    fn insert_phrases_file() {
        let wtr = io::BufWriter::new(File::create("/tmp/phrase-set.fst").unwrap());

        let mut build = PhraseSetBuilder::new(wtr).unwrap();
        build.insert(&[1u64, 61_528_u64, 561_528u64]).unwrap();
        build.insert(&[61_528_u64, 561_528u64, 1u64]).unwrap();
        build.insert(&[561_528u64, 1u64, 61_528_u64]).unwrap();
        build.finish().unwrap();

        let phrase_set = unsafe { PhraseSet::from_path("/tmp/phrase-set.fst") }.unwrap();

        let mut keys = vec![];
        let mut stream = phrase_set.into_stream();
        while let Some(key) = stream.next() {
            keys.push(key.to_vec());
        }
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

    #[test]
    fn contains_query() {
        let mut build = PhraseSetBuilder::memory();
        build.insert(&[1u64, 61_528_u64, 561_528u64]).unwrap();
        build.insert(&[61_528_u64, 561_528u64, 1u64]).unwrap();
        build.insert(&[561_528u64, 1u64, 61_528_u64]).unwrap();
        let bytes = build.into_inner().unwrap();

        let phrase_set = PhraseSet::from_bytes(bytes).unwrap();

        let phrase_one = [1u64, 61_528u64, 561_528u64];
        assert_eq!(true, phrase_set.contains(&phrase_one));

        let phrase_none = [1u64, 1u64, 1u64];
        assert_eq!(false, phrase_set.contains(&phrase_none));
    }
}

