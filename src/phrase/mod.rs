pub mod util;
pub mod query;

use std::io;
#[cfg(feature = "mmap")]
use std::path::Path;

use fst;
use fst::{IntoStreamer, Set, SetBuilder};
use fst::raw::{CompiledAddr};

use self::util::word_ids_to_key;
use self::util::PhraseSetError;
use self::query::{QueryPhrase};

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

    /// Test membership of a single phrase. Returns true iff the phrase matches a complete phrase
    /// in the set. Wraps the underlying Set::contains method.
    pub fn contains(&self, phrase: QueryPhrase) -> Result<bool, PhraseSetError> {
        if phrase.has_prefix {
            return Err(PhraseSetError::new("The query submitted has a QueryWord::Prefix. Set::contains only accepts QueryWord:Full"));
        }
        let key = phrase.full_word_key();
        Ok(self.0.contains(key))
    }

    /// Test whether a query phrase can be found at the beginning of any phrase in the Set. Also
    /// known as a "starts with" search.
    pub fn contains_prefix(&self, phrase: QueryPhrase) -> Result<bool, PhraseSetError>  {
        if phrase.has_prefix {
            match self.contains_prefix_with_range(phrase) {
                true => return Ok(true),
                false => return Ok(false),
            }
        }
        let key = phrase.full_word_key();
        let fst = self.0.as_fst();
        let root_addr = fst.root().addr();
        match self.partial_search(root_addr, &key) {
            None => return Ok(false),
            Some(..) => return Ok(true),
        }
    }

    /// Helper function for doing a byte-by-byte walk through the phrase graph, staring at any
    /// arbitrary node. Not to be used directly.
    fn partial_search(&self, start_addr: CompiledAddr, key: &[u8]) -> Option<CompiledAddr> {
        let fst = self.0.as_fst();
        let mut node = fst.node(start_addr);
        // move through the tree byte by byte
        for b in key {
            node = match node.find_input(*b) {
                None => return None,
                Some(i) => fst.node(node.transition_addr(i)),
            }
        }
        return Some(node.addr())
    }

    // TODO: this needs to get called inside contains_prefix when final word is QueryWord::prefix <15-05-18, boblannon> //
    fn contains_prefix_with_range(&self, phrase: QueryPhrase) -> bool {
        let (sought_min_key, sought_max_key) = phrase.prefix_key_range().unwrap();

		// self as fst
        let fst = &self.0.as_fst();
        // start from root node
        let root_node = fst.root();
        println!("root is {:?}", root_node );

		// using the keys for the full words, walk the graph. if no path accepts these keys, stop
        // here. result node should not be final.
        let full_word_key = phrase.full_word_key();
        let full_word_addr = match self.partial_search(root_node.addr(), &full_word_key) {
            None => {
                return false
            },
            Some(addr) => {
                let full_word_node = fst.node(addr);
                // since we still have a prefix to evaluate, we shouldn't have arrived at a node
                // with zero transitions. if so, we know the prefix won't match.
                if full_word_node.is_empty() {
                    return false
                } else {
                    full_word_node.addr()
                }
            }
        };
        // does the key at the low end of the prefix range take us to a final state? if so, we know
        // that at least one of the possible phrases is in the graph
        match self.partial_search(full_word_addr, &sought_min_key) {
            Some(..) => { return true },
            _ => (),
        }

        // does the key at the high end of the prefix range take us to a final state? if so, we know
        // that at least one of the possible phrases is in the graph
        match self.partial_search(full_word_addr, &sought_max_key) {
            Some(..) => { return true },
            _ => (),
        }

        // get actual_min
        let mut min_n = fst.node(full_word_addr);
        let mut actual_min_key: Vec<u8> = Vec::new();
        for _i in 0..3 {
            let min_t = min_n.transitions().nth(0).unwrap();
            actual_min_key.push(min_t.inp);
            min_n = fst.node(min_t.addr);
        }

        debug_assert!(actual_min_key.len() == 3);

        // if actual_min > sought_max: sought range is below actual range
        if actual_min_key > sought_max_key {
            return false
        }
        // these two can be collapsed, assuming actual_min < sought_max:
        //   - if (actual_min > sought_min) && (actual_max > sought_max): true
        //   - if (actual_min > sought_min) && (actual_max < sought_max): true
        // └─> else if (actual_min > sought_min): true
        else if actual_min_key > sought_min_key {
            return true
        }

        // get actual_max
        let mut max_n = fst.node(full_word_addr);
        let mut actual_max_key: Vec<u8> = Vec::new();
        for _i in 0..3 {
            let max_t = max_n.transitions().last().unwrap();
            actual_max_key.push(max_t.inp);
            max_n = fst.node(max_t.addr);
        }

        debug_assert!(actual_max_key.len() == 3);

        // if actual_max < sought_min: sought range is above actual range
        if actual_max_key < sought_min_key {
            return false
        }

        // by now we know that the ranges intersect.
        // if actual_max < sought_max: the intersection includes actual_max, so we know there's at
        // least one match
        if actual_max_key < sought_max_key {
            return true
        }

        // Now we know that the sought range is completely contained within the limits of the
        // actual one. We're still not sure, though, if there is an actual path shared by both. A
        // simple, non-graph example to demonstrate this point:
        //
        // ```
        // let actual_values = [ 1, 2, 7, 8, 9 ]
        // let actual_range = (actual_values.first(), actual_values.last())
        // let sought_range = (3, 6)
        // ```
        //
        // Here, `sought_range` is completely contained within `actual_range` (1,9)`), but that
        // doesn't mean that there's actually a match.  we have to look at actual values and ask
        // whether or not any of them really fall within the sought range. Since none do, we'd
        // return false.
        debug_assert!((actual_min_key < sought_min_key) && (actual_max_key > sought_max_key));

        // The same is true here, so we need to look for any evidence that there's at least one
        // valid path in the graph that is within our sought range. We need to traverse the subtree
        // bounded by the prefix range, if it exists. We know that `sought_min_key` and
        // `sought_max_key` aren't in the graph, but parts of them may be.
        //
        for i in 0..3 {
            ();
            // Each iteration `i` of this loop will first check to see if there is any transition `t`
            // from the current node whose acceptance input (`t.inp`) is between `sought_min_key[i]`
            // and `sought_max_key[i]`. If there is, we know for sure that there's at least one path
            // within the sought range, so `return true`.
            //
            // If not, we try to travel forward in the graph by one byte along one or both of:
            //   a. the transition `t` where `t.inp=sought_min_key[i]`
            //   b. the transition `t` where `t.inp=sought_max_key[i]`
            //
            // If both of these are unavailable, there's definitely not a sought path within the actual
            // range. Return false.
            //
            // If only (a) is available, continue to the next iteration, but from now on we'll look for
            // any `t.inp > sought_min_key[i]`.
            //
            // Conversely: if only (b) is available, continue to the next iteration, but from now on
            // we'll look for any `t.inp < sought_max_key[i]`.
        }

        return false
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
    pub fn insert(&mut self, phrase: &[u32]) -> Result<(), fst::Error> {
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
    use self::query::{QueryPhrase, QueryWord};
    use self::util::three_byte_decode;

    #[test]
    fn insert_phrases_memory() {
        let mut build = PhraseSetBuilder::memory();
        build.insert(&[1u32, 61_528_u32, 561_528u32]).unwrap();
        build.insert(&[61_528_u32, 561_528u32, 1u32]).unwrap();
        build.insert(&[561_528u32, 1u32, 61_528_u32]).unwrap();
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
        build.insert(&[1u32, 61_528_u32, 561_528u32]).unwrap();
        build.insert(&[61_528_u32, 561_528u32, 1u32]).unwrap();
        build.insert(&[561_528u32, 1u32, 61_528_u32]).unwrap();
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
        build.insert(&[1u32, 61_528_u32, 561_528u32]).unwrap();
        build.insert(&[61_528_u32, 561_528u32, 1u32]).unwrap();
        build.insert(&[561_528u32, 1u32, 61_528_u32]).unwrap();
        let bytes = build.into_inner().unwrap();

        let phrase_set = PhraseSet::from_bytes(bytes).unwrap();

        let words = vec![
            QueryWord::Full{ id: 1u32, edit_distance: 0 },
            QueryWord::Full{ id: 61_528u32, edit_distance: 0 },
            QueryWord::Full{ id: 561_528u32, edit_distance: 0 },
        ];

        let matching_word_seq = [ words[0], words[1], words[2] ];
        let matching_phrase = QueryPhrase::new(&matching_word_seq).unwrap();
        assert_eq!(true, phrase_set.contains(matching_phrase).unwrap());

        let missing_word_seq = [ words[0], words[1] ];
        let missing_phrase = QueryPhrase::new(&missing_word_seq).unwrap();
        assert_eq!(false, phrase_set.contains(missing_phrase).unwrap());

        let prefix = QueryWord::Prefix{ id_range: (561_528u32, 561_531u32) };
        let has_prefix_word_seq = [ words[0], words[1], prefix ];
        let has_prefix_phrase = QueryPhrase::new(&has_prefix_word_seq).unwrap();
        assert!(phrase_set.contains(has_prefix_phrase).is_err());
    }

    #[test]
    fn contains_prefix_query() {
        let mut build = PhraseSetBuilder::memory();
        build.insert(&[1u32, 61_528_u32, 561_528u32]).unwrap();
        build.insert(&[61_528_u32, 561_528u32, 1u32]).unwrap();
        build.insert(&[561_528u32, 1u32, 61_528_u32]).unwrap();
        let bytes = build.into_inner().unwrap();

        let phrase_set = PhraseSet::from_bytes(bytes).unwrap();

        let words = vec![
            QueryWord::Full{ id: 1u32, edit_distance: 0 },
            QueryWord::Full{ id: 61_528u32, edit_distance: 0 },
            QueryWord::Full{ id: 561_528u32, edit_distance: 0 },
        ];

        let matching_word_seq = [ words[0], words[1] ];
        let matching_phrase = QueryPhrase::new(&matching_word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(matching_phrase).unwrap());

        let missing_word_seq = [ words[0], words[2] ];
        let missing_phrase = QueryPhrase::new(&missing_word_seq).unwrap();
        assert_eq!(false, phrase_set.contains_prefix(missing_phrase).unwrap());
    }

    #[test]
    fn contains_prefix_range() {
        // This is where we'll write our map to.
        let wtr = io::BufWriter::new(File::create("map.fst").unwrap());
        let mut build = PhraseSetBuilder::new(wtr).unwrap();
        // let mut build = PhraseSetBuilder::memory();
        build.insert(&[1u32, 61_528_u32, three_byte_decode(&[2u8, 1u8, 0u8])]).unwrap();
        build.insert(&[1u32, 61_528_u32, three_byte_decode(&[2u8, 3u8, 2u8])]).unwrap();
        build.insert(&[1u32, 61_528_u32, three_byte_decode(&[2u8, 3u8, 4u8])]).unwrap();
        build.insert(&[1u32, 61_528_u32, three_byte_decode(&[2u8, 5u8, 6u8])]).unwrap();
        build.insert(&[1u32, 61_528_u32, three_byte_decode(&[4u8, 1u8, 1u8])]).unwrap();
        build.insert(&[1u32, 61_528_u32, three_byte_decode(&[4u8, 3u8, 3u8])]).unwrap();
        build.insert(&[1u32, 61_528_u32, three_byte_decode(&[4u8, 5u8, 5u8])]).unwrap();
        build.insert(&[1u32, 61_528_u32, three_byte_decode(&[6u8, 3u8, 4u8])]).unwrap();
        build.insert(&[1u32, 61_528_u32, three_byte_decode(&[6u8, 3u8, 7u8])]).unwrap();
        build.insert(&[1u32, 61_528_u32, three_byte_decode(&[6u8, 5u8, 8u8])]).unwrap();
        build.finish().unwrap();

        let phrase_set = unsafe { PhraseSet::from_path("map.fst").unwrap() };
        // let bytes = build.into_inner().unwrap();
        // let phrase_set = PhraseSet::from_bytes(bytes).unwrap();

        let words = vec![
            QueryWord::Full{ id: 1u32, edit_distance: 0 },
            QueryWord::Full{ id: 61_528u32, edit_distance: 0 },
            QueryWord::Full{ id: 561_528u32, edit_distance: 0 },
        ];

        // matches and the min edge of range
        let matching_prefix_min = QueryWord::Prefix{ id_range: (
                three_byte_decode(&[6u8, 5u8, 8u8]),
                three_byte_decode(&[255u8, 255u8, 255u8]),
                ) };
        let word_seq = [ words[0], words[1], matching_prefix_min ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // matches at the max edge of range
        let matching_prefix_max = QueryWord::Prefix{ id_range: (
                three_byte_decode(&[0u8, 0u8, 0u8]),
                three_byte_decode(&[2u8, 1u8, 0u8]),
                ) };
        let word_seq = [ words[0], words[1], matching_prefix_max ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // range is larger than possible outcomes
        let matching_prefix_larger = QueryWord::Prefix{ id_range: (
                three_byte_decode(&[2u8, 0u8, 255u8]),
                three_byte_decode(&[6u8, 5u8, 1u8]),
                ) };
        let word_seq = [ words[0], words[1], matching_prefix_larger ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // high side of range overlaps
        let matching_prefix_hi = QueryWord::Prefix{ id_range: (
                three_byte_decode(&[0u8, 0u8, 0u8]),
                three_byte_decode(&[2u8, 2u8, 1u8]),
                ) };
        let word_seq = [ words[0], words[1], matching_prefix_hi ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // low side of range overlaps
        let matching_prefix_low = QueryWord::Prefix{ id_range: (
                three_byte_decode(&[6u8, 4u8, 1u8]),
                three_byte_decode(&[255u8, 255u8, 255u8]),
                ) };
        let word_seq = [ words[0], words[1], matching_prefix_low ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // no overlap, too low
        let missing_prefix_low = QueryWord::Prefix{ id_range: (
                three_byte_decode(&[0u8, 0u8, 0u8]),
                three_byte_decode(&[2u8, 0u8, 255u8]),
                ) };
        let word_seq = [ words[0], words[1], missing_prefix_low ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(false, phrase_set.contains_prefix(phrase).unwrap());

        // no overlap, too high
        let missing_prefix_hi = QueryWord::Prefix{ id_range: (
                three_byte_decode(&[6u8, 5u8, 9u8]),
                three_byte_decode(&[255u8, 255u8, 255u8]),
                ) };
        let word_seq = [ words[0], words[1], missing_prefix_hi ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(false, phrase_set.contains_prefix(phrase).unwrap());

    }

    // fn contains_prefix_range() {
    //     // This is where we'll write our map to.
    //     let wtr = io::BufWriter::new(File::create("map.fst").unwrap());
    //     let mut build = PhraseSetBuilder::new(wtr).unwrap();
    //     // let mut build = PhraseSetBuilder::memory();
    //     build.insert(&[1u32, 61_528_u32, three_byte_decode(&[2u8, 1u8, 0u8]), 345u32]).unwrap();
    //     build.insert(&[1u32, 61_528_u32, three_byte_decode(&[2u8, 3u8, 2u8]), 345u32]).unwrap();
    //     build.insert(&[1u32, 61_528_u32, three_byte_decode(&[2u8, 3u8, 4u8]), 345u32]).unwrap();
    //     build.insert(&[1u32, 61_528_u32, three_byte_decode(&[2u8, 5u8, 6u8]), 345u32]).unwrap();
    //     build.insert(&[1u32, 61_528_u32, three_byte_decode(&[4u8, 1u8, 1u8]), 345u32]).unwrap();
    //     build.insert(&[1u32, 61_528_u32, three_byte_decode(&[4u8, 3u8, 3u8]), 345u32]).unwrap();
    //     build.insert(&[1u32, 61_528_u32, three_byte_decode(&[4u8, 5u8, 5u8]), 345u32]).unwrap();
    //     build.insert(&[1u32, 61_528_u32, three_byte_decode(&[6u8, 3u8, 4u8]), 345u32]).unwrap();
    //     build.insert(&[1u32, 61_528_u32, three_byte_decode(&[6u8, 3u8, 7u8]), 345u32]).unwrap();
    //     build.insert(&[1u32, 61_528_u32, three_byte_decode(&[6u8, 5u8, 8u8]), 345u32]).unwrap();
    //     build.finish().unwrap();
}

