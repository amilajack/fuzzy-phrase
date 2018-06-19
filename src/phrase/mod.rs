pub mod util;
pub mod query;

use std::io;
#[cfg(feature = "mmap")]
use std::path::Path;

use fst;
use fst::{IntoStreamer, Set, SetBuilder, Streamer};
use fst::raw::{CompiledAddr, Node};

use self::util::{word_ids_to_key};
use self::util::PhraseSetError;
use self::query::{QueryPhrase, QueryWord};

type WordKey = [u8; 3];

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
        let key = phrase.full_word_key();
        let fst = self.0.as_fst();
        let root_addr = fst.root().addr();
        match self.partial_search(root_addr, &key) {
            None => Ok(false),
            Some(addr) => {
                if phrase.has_prefix {
                    Ok(self.matches_prefix_range(addr, phrase.prefix_key_range().unwrap()))
                } else {
                    Ok(true)
                }
            }
        }
    }

    /// Recursively explore the phrase graph looking for combinations of candidate words to see
    /// which ones match actual phrases in the phrase graph.
    pub fn match_combinations(
        &self,
        word_possibilities: &[Vec<QueryWord>],
        max_phrase_dist: u8
    ) -> Result<Vec<Vec<QueryWord>>, PhraseSetError> {
        // this is just a thin wrapper around a private recursive function, with most of the
        // arguments prefilled
        let fst = self.0.as_fst();
        let root = fst.root();
        let mut out: Vec<Vec<QueryWord>> = Vec::new();
        self.exact_recurse(word_possibilities, 0, &root, max_phrase_dist, Vec::new(), &mut out)?;
        Ok(out)
    }

    fn exact_recurse(
        &self,
        possibilities: &[Vec<QueryWord>],
        position: usize,
        node: &Node,
        budget_remaining: u8,
        so_far: Vec<QueryWord>,
        out: &mut Vec<Vec<QueryWord>>,
    ) -> Result<(), PhraseSetError> {
        let fst = self.0.as_fst();

        for word in possibilities[position].iter() {
            let (key, edit_distance) = match word {
                QueryWord::Full { key, edit_distance, .. } => (*key, *edit_distance),
                _ => return Err(PhraseSetError::new(
                    "The query submitted has a QueryWord::Prefix. Set::contains only accepts QueryWord:Full"
                )),
            };
            if edit_distance > budget_remaining {
                break
            }

            // can we find the next word from our current position?
            let mut found = true;
            // make a mutable copy to traverse
            let mut search_node = node.to_owned();
            for b in key.into_iter() {
                if let Some(i) = search_node.find_input(*b) {
                    search_node = fst.node(search_node.transition_addr(i));
                } else {
                    found = false;
                    break;
                }
            }

            // only recurse or add a result if we the current word is in the graph in this position
            if found {
                let mut rec_so_far = so_far.clone();
                rec_so_far.push(word.clone());
                if position < possibilities.len() - 1 {
                    self.exact_recurse(
                        possibilities,
                        position + 1,
                        &search_node,
                        budget_remaining - edit_distance,
                        rec_so_far,
                        out,
                    )?;
                } else {
                    // if we're at the end of the line, we'll only keep this result if it's final
                    if search_node.is_final() {
                        out.push(rec_so_far);
                    }
                }
            }
        }
        Ok(())
    }

    /// Recursively explore the phrase graph looking for combinations of candidate words to see
    /// which ones match prefixes of actual phrases in the phrase graph.
    pub fn match_combinations_as_prefixes(
        &self,
        word_possibilities: &[Vec<QueryWord>],
        max_phrase_dist: u8
    ) -> Result<Vec<Vec<QueryWord>>, PhraseSetError> {
        // this is just a thin wrapper around a private recursive function, with most of the
        // arguments prefilled
        let fst = self.0.as_fst();
        let root = fst.root();
        let mut out: Vec<Vec<QueryWord>> = Vec::new();
        self.prefix_recurse(word_possibilities, 0, &root, max_phrase_dist, Vec::new(), &mut out)?;
        Ok(out)
    }

    fn prefix_recurse(
        &self,
        possibilities: &[Vec<QueryWord>],
        position: usize,
        node: &Node,
        budget_remaining: u8,
        so_far: Vec<QueryWord>,
        out: &mut Vec<Vec<QueryWord>>,
    ) -> Result<(), PhraseSetError> {
        let fst = self.0.as_fst();

        for word in possibilities[position].iter() {
            match word {
                QueryWord::Full { key, edit_distance, .. } => {
                    if *edit_distance > budget_remaining {
                        break
                    }

                    let mut found = true;
                    // make a mutable copy to traverse
                    let mut search_node = node.to_owned();
                    for b in key.into_iter() {
                        if let Some(i) = search_node.find_input(*b) {
                            search_node = fst.node(search_node.transition_addr(i));
                        } else {
                            found = false;
                            break;
                        }
                    }

                    // only recurse or add a result if we the current word is in the graph in
                    // this position
                    if found {
                        let mut rec_so_far = so_far.clone();
                        rec_so_far.push(word.clone());
                        if position < possibilities.len() - 1 {
                            self.prefix_recurse(
                                possibilities,
                                position + 1,
                                &search_node,
                                budget_remaining - edit_distance,
                                rec_so_far,
                                out,
                            )?;
                        } else {
                            out.push(rec_so_far);
                        }
                    }
                },
                QueryWord::Prefix { key_range, .. } => {
                    if self.matches_prefix_range(
                        node.addr(),
                        *key_range
                    ) {
                        // presumably the prefix is at the end, so we don't need to consider the
                        // possibility of recursing, just of being done
                        let mut rec_so_far = so_far.clone();
                        rec_so_far.push(word.clone());
                        out.push(rec_so_far);
                    }
                },
            }
        }
        Ok(())
    }

    /// Recursively explore the phrase graph looking for combinations of candidate words to see
    /// which ones match prefixes of actual phrases in the phrase graph.
    pub fn match_combinations_as_windows(
        &self,
        word_possibilities: &[Vec<QueryWord>],
        max_phrase_dist: u8,
        ends_in_prefix: bool
    ) -> Result<Vec<(Vec<QueryWord>, bool)>, PhraseSetError> {
        // this is just a thin wrapper around a private recursive function, with most of the
        // arguments prefilled
        let fst = self.0.as_fst();
        let root = fst.root();
        let mut out: Vec<(Vec<QueryWord>, bool)> = Vec::new();
        self.window_recurse(word_possibilities, 0, &root, max_phrase_dist, ends_in_prefix, Vec::new(), &mut out)?;
        Ok(out)
    }

    fn window_recurse(
        &self,
        possibilities: &[Vec<QueryWord>],
        position: usize,
        node: &Node,
        budget_remaining: u8,
        ends_in_prefix: bool,
        so_far: Vec<QueryWord>,
        out: &mut Vec<(Vec<QueryWord>, bool)>,
    ) -> Result<(), PhraseSetError> {
        let fst = self.0.as_fst();

        for word in possibilities[position].iter() {
            match word {
                QueryWord::Full { key, edit_distance, .. } => {
                    if *edit_distance > budget_remaining {
                        break
                    }

                    let mut found = true;
                    // make a mutable copy to traverse
                    let mut search_node = node.to_owned();
                    for b in key.into_iter() {
                        if let Some(i) = search_node.find_input(*b) {
                            search_node = fst.node(search_node.transition_addr(i));
                        } else {
                            found = false;
                            break;
                        }
                    }

                    // only recurse or add a result if we the current word is in the graph in
                    // this position
                    if found {
                        // we want to add a result if we're at the end OR if we've hit a final
                        // node OR we're at the end of the phrase
                        let mut rec_so_far = so_far.clone();
                        rec_so_far.push(word.clone());
                        if position < possibilities.len() - 1 {
                            if search_node.is_final() {
                                out.push((rec_so_far.clone(), false));
                            }
                            self.window_recurse(
                                possibilities,
                                position + 1,
                                &search_node,
                                budget_remaining - edit_distance,
                                ends_in_prefix,
                                rec_so_far,
                                out,
                            )?;
                        } else {
                            // if we're at the end, require final node unless autocomplete is on
                            if search_node.is_final() || ends_in_prefix {
                                out.push((rec_so_far, ends_in_prefix));
                            }
                        }
                    }
                },
                QueryWord::Prefix { key_range, .. } => {
                    if self.matches_prefix_range(
                        node.addr(),
                        *key_range
                    ) {
                        // presumably the prefix is at the end, so we don't need to consider the
                        // possibility of recursing, just of being done; we can also assume AC is on
                        let mut rec_so_far = so_far.clone();
                        rec_so_far.push(word.clone());
                        out.push((rec_so_far, ends_in_prefix));
                    }
                },
            }
        }
        Ok(())
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

    fn matches_prefix_range(&self, start_position: CompiledAddr, key_range: (WordKey, WordKey)) -> bool {
        let (sought_min_key, sought_max_key) = key_range;

		// self as fst
        let fst = &self.0.as_fst();

        // get min value greater than or qual to the sought min
        let node0 = fst.node(start_position);
        for t0 in node0.transitions().skip_while(|t| t.inp < sought_min_key[0]) {
            let must_skip1 = t0.inp == sought_min_key[0];
            let node1 = fst.node(t0.addr);
            for t1 in node1.transitions() {
                if must_skip1 && t1.inp < sought_min_key[1] {
                    continue;
                }
                let must_skip2 = must_skip1 && t1.inp == sought_min_key[1];
                let node2 = fst.node(t1.addr);
                for t2 in node2.transitions() {
                    if must_skip2 && t2.inp < sought_min_key[2] {
                        continue;
                    }
                    // we've got three bytes! woohoo!
                    let mut next_after_min = [t0.inp, t1.inp, t2.inp];
                    return next_after_min <= sought_max_key;
                }
            }
        }
        false
    }

    pub fn range(&self, phrase: QueryPhrase) -> Result<bool, PhraseSetError> {
        let mut max_key = phrase.full_word_key();
        let mut min_key = phrase.full_word_key();
        let (last_id_min, last_id_max) = phrase.prefix_key_range().unwrap();
        min_key.extend_from_slice(&last_id_min);
        max_key.extend_from_slice(&last_id_max);
        let mut range_stream = self.0.range().ge(min_key).le(max_key).into_stream();
        let _result = match range_stream.next() {
            Some(..) => return Ok(true),
            None => return Ok(false),
        };
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
            QueryWord::new_full(1u32, 0),
            QueryWord::new_full(61_528u32, 0),
            QueryWord::new_full(561_528u32, 0),
        ];

        let matching_word_seq = [ words[0], words[1], words[2] ];
        let matching_phrase = QueryPhrase::new(&matching_word_seq).unwrap();
        assert_eq!(true, phrase_set.contains(matching_phrase).unwrap());

        let missing_word_seq = [ words[0], words[1] ];
        let missing_phrase = QueryPhrase::new(&missing_word_seq).unwrap();
        assert_eq!(false, phrase_set.contains(missing_phrase).unwrap());

        let prefix = QueryWord::new_prefix((561_528u32, 561_531u32));
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
            QueryWord::new_full(1u32, 0),
            QueryWord::new_full(61_528u32,  0),
            QueryWord::new_full(561_528u32, 0),
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
        let mut build = PhraseSetBuilder::memory();
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
        let bytes = build.into_inner().unwrap();
        let phrase_set = PhraseSet::from_bytes(bytes).unwrap();

        let words = vec![
            QueryWord::new_full(1u32,       0 ),
            QueryWord::new_full(61_528u32,  0 ),
            QueryWord::new_full(561_528u32, 0 ),
        ];

        // matches and the min edge of range
        let prefix_id_range = (
            three_byte_decode(&[6u8, 5u8, 8u8]),
            three_byte_decode(&[255u8, 255u8, 255u8]));
        let matching_prefix_min = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], matching_prefix_min ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // matches at the max edge of range
        let prefix_id_range = (
                three_byte_decode(&[0u8, 0u8, 0u8]),
                three_byte_decode(&[2u8, 1u8, 0u8]));
        let matching_prefix_max = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], matching_prefix_max ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // range is larger than possible outcomes
        let prefix_id_range = (
                three_byte_decode(&[2u8, 0u8, 255u8]),
                three_byte_decode(&[6u8, 5u8, 1u8]));
        let matching_prefix_larger = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], matching_prefix_larger ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // high side of range overlaps
        let prefix_id_range = (
                three_byte_decode(&[0u8, 0u8, 0u8]),
                three_byte_decode(&[2u8, 2u8, 1u8]));
        let matching_prefix_hi = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], matching_prefix_hi ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // low side of range overlaps
        let prefix_id_range = (
                three_byte_decode(&[6u8, 4u8, 1u8]),
                three_byte_decode(&[255u8, 255u8, 255u8]));
        let matching_prefix_low = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], matching_prefix_low ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // no overlap, too low
        let prefix_id_range = (
                three_byte_decode(&[0u8, 0u8, 0u8]),
                three_byte_decode(&[2u8, 0u8, 255u8]));
        let missing_prefix_low = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], missing_prefix_low ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(false, phrase_set.contains_prefix(phrase).unwrap());

        // no overlap, too high
        let prefix_id_range = (
                three_byte_decode(&[6u8, 5u8, 9u8]),
                three_byte_decode(&[255u8, 255u8, 255u8]));
        let missing_prefix_hi = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], missing_prefix_hi ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(false, phrase_set.contains_prefix(phrase).unwrap());

    }

    #[test]
    fn contains_prefix_nested_range() {
        // in each of these cases, the sought range is within actual range, but the min and max
        // keys are not in the graph. that means we need to make sure that there is at least one
        // path that is actually within the sought range.
        let mut build = PhraseSetBuilder::memory();
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
        let bytes = build.into_inner().unwrap();
        let phrase_set = PhraseSet::from_bytes(bytes).unwrap();

        let words = vec![
            QueryWord::new_full(1u32,       0 ),
            QueryWord::new_full(61_528u32,  0 ),
            QueryWord::new_full(561_528u32, 0 ),
        ];

        // matches because (4, 3, 3) is in range
        let prefix_id_range = (
                three_byte_decode(&[4u8, 3u8, 1u8]),
                three_byte_decode(&[4u8, 3u8, 5u8]));
        let matching_two_bytes = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], matching_two_bytes ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // does not match because there is no actual path in sought range.
        let prefix_id_range = (
                three_byte_decode(&[4u8, 3u8, 0u8]),
                three_byte_decode(&[4u8, 3u8, 2u8]),
                ) ;
        let missing_two_bytes = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], missing_two_bytes ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(false, phrase_set.contains_prefix(phrase).unwrap());

        // matches because (4, 1, 1) is in range
        let prefix_id_range = (
                three_byte_decode(&[4u8, 0u8, 1u8]),
                three_byte_decode(&[4u8, 2u8, 5u8]),
                ) ;
        let matching_one_byte = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], matching_one_byte ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // does not match because there is no actual path in sought range.
        let prefix_id_range = (
                three_byte_decode(&[4u8, 4u8, 0u8]),
                three_byte_decode(&[4u8, 5u8, 2u8]),
                ) ;
        let missing_one_byte = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], missing_one_byte ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(false, phrase_set.contains_prefix(phrase).unwrap());

        // matches because (2, 5, 6) is in range. gives up searching high path because 0 is not in
        // the transitions for the byte after 4, which are [1, 3, 5].
        let prefix_id_range = (
                three_byte_decode(&[2u8, 4u8, 1u8]),
                three_byte_decode(&[4u8, 0u8, 0u8]),
                ) ;
        let matching_one_byte_lo = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], matching_one_byte_lo ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // misses because nothing is in range. gives up searching high path because 0 is not in
        // the transitions for the byte after 4, which are [1, 3, 5].
        let prefix_id_range = (
                three_byte_decode(&[2u8, 6u8, 1u8]),
                three_byte_decode(&[4u8, 0u8, 0u8]),
                ) ;
        let missing_one_byte_lo = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], missing_one_byte_lo ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(false, phrase_set.contains_prefix(phrase).unwrap());

        // matches because (6, 3, 4) is in range. gives up searching low path because 7 is not in
        // the transitions for the byte after 4, which are [1, 3, 5].
        let prefix_id_range = (
                three_byte_decode(&[4u8, 7u8, 1u8]),
                three_byte_decode(&[6u8, 4u8, 0u8]),
                ) ;
        let matching_one_byte_hi = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], matching_one_byte_hi ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // misses because nothing is in range. gives up searching low path because 7 is not in
        // the transitions for the byte after 4, which are [1, 3, 5].
        let prefix_id_range = (
                three_byte_decode(&[4u8, 7u8, 1u8]),
                three_byte_decode(&[6u8, 2u8, 0u8]),
                ) ;
        let missing_one_byte_hi = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], missing_one_byte_hi ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(false, phrase_set.contains_prefix(phrase).unwrap());

        // matches because (2, 1, 0) is on the low edge of the actual range, but sought range has
        // same min and max
        let prefix_id_range = (
                three_byte_decode(&[2u8, 1u8, 0u8]),
                three_byte_decode(&[2u8, 1u8, 0u8]),
                ) ;
        let matching_edge_low = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], matching_edge_low ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());

        // matches because (2, 1, 0) is on the low edge of the actual range, but sought range has
        // same min and max
        let prefix_id_range = (
                three_byte_decode(&[6u8, 5u8, 8u8]),
                three_byte_decode(&[6u8, 5u8, 8u8]),
                ) ;
        let matching_edge_hi = QueryWord::new_prefix(prefix_id_range);
        let word_seq = [ words[0], words[1], matching_edge_hi ];
        let phrase = QueryPhrase::new(&word_seq).unwrap();
        assert_eq!(true, phrase_set.contains_prefix(phrase).unwrap());


    }

}

