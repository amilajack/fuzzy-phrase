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
            return Err(PhraseSetError::new("The query submitted has a QueryWord::Prefix. Set::contains_prefix only accepts QueryWord:Full"));
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

    fn contains_with_range(&self, phrase: QueryPhrase) -> bool {
        let (prefix_min_key, prefix_max_key) = phrase.prefix_key_range().unwrap();

		// self as fst
        let fst = &self.0.as_fst();
        // start from root node
        let root_node = fst.root();

		// using the keys for the full words, walk the graph. if no path accepts these keys, stop
        // here. result node should not be final.
        let full_word_key = phrase.full_word_key();
        let full_word_addr = match self.partial_search(root_node.addr(), full_word_key) {
            None => return false,
            Some(addr) => {
                let full_word_node = fst.node(addr);
                // since we still have a prefix to evaluate, we shouldn't have arrived at a node
                // with zero transitions. if so, we know the prefix won't match.
                if full_word_node.is_empty() {
                    return false
                } else {
                    addr
                }
            }
        };

        // does the key at the low end of the prefix range take us to a final state? if so, we know
        // that at least one of the possible phrases is in the graph
        match self.partial_search(full_word_addr, prefix_min_key) {
            Some(addr) => {
                let prefix_min_node = fst.node(addr);
                if prefix_min_node.is_final() {
                    return true
                }
            },
            _ => (),
        }

        // does the key at the high end of the prefix range take us to a final state? if so, we know
        // that at least one of the possible phrases is in the graph
        match self.partial_search(full_word_addr, prefix_max_key) {
            Some(addr) => {
                let prefix_min_node = fst.node(addr);
                if prefix_min_node.is_final() {
                    return true
                }
            },
            _ => (),
        }

		// if we're still not sure, we need to traverse the subtree bounded by the prefix range.
        // Each iteration of the loop works like this:
        //   (1) try to walk to the next node in the path of the min and max keys
        //   (2) collect all of the nodes pointed to by transitions above the min path and below
        //   the max path
        //   (3) look at all of the nodes collected in (2) and determine whether they have
        //   children that are final states.
        let mut min_bound = full_word_addr;
        let mut max_bound = full_word_addr;
        // going byte-by-byte in the prefix min/max keys (each of which is three bytes)
        for i in 0..3 {
            // each iteration, we'll have to capture and explore the transitions whose inputs are
            // between the ith byte of the min key and the ith byte of the max key. there's
            // going to be overlap here, particularly for i=0, so we use a set to avoid duplicate
            // effort.
            let mut middle: HashSet::new();

            // if the previous iteration found a new node on the min key's path:
            if min_bound != None {
                // find the node
                let min_bound_node = fst.node(min_bound);
                // select all of the transitions whose input is greater than the min key's ith byte
                for t in min_bound_node.transitions().filter(|t| t.inp > prefix_min_key[i]) {
                    // in the first iteration, the min and max bounds are the same, so we
                    // need to avoid transitions that are above the max bound
                    if (i > 0) || (t.inp < prefix_max_key[i]) {
                        middle.insert(t.addr);
                    }
                }

                // for the next iteration, try to walk to the next node on the min key's path
                let min_bound = match min_bound_node.find_input(prefix_min_key[i]) {
                    None => None,
                    Some(a) => min_bound_node.transition_addr(a),
                };
            }

            // if the previous iteration found a new node on the max key's path:
            if max_bound != None {
                // find the node
                let max_bound_node = fst.node(max_bound);
                // select all of the transitions whose input is less than the max key's ith byte
                for t in max_bound_node.transitions().filter(|t| t.inp < prefix_max_key[i]) {
                    // in the first iteration, the min and max bounds are the same, so we
                    // need to avoid transitions that are below the min bound
                    if (i > 0) || (t.inp > prefix_min_key[i]) {
                        middle.insert(t.addr);
                    }
                }

                // for the next iteration, try to walk to the next node on the max key's path
                let max_bound = match max_bound_node.find_input(prefix_max_key[i]) {
                    None => None,
                    Some(a) => fst.node(max_bound.transition_addr(a)),
                };
            }

            // For each node in the middle, we can be sure that walking another 1 or 2 bytes will
            // be within the range of the min and max prefix keys. What we don't know is if that
            // will take us to a final state. Here we'll check for that.
            //
            // The depth of the sort will always be 2 minus the iteration we're on.
            //  - for i = 0, we've gone one byte in and need to go two more
            //  - for i = 1, we've gone two bytes in and need to go one more
            //  - for i = 2, we've gone three bytes in and just need to check where we're at
            let depth = 2 - i;
            for m in &middle {
                match self.final_at_depth(m, depth) {
                    true => return true,
                    _ => (),
                }
            }
        }

        return true
    }

    /// Search the children of a node up to some depth and determine whether any of them is a final
    /// state.
    fn final_at_depth(&self, addr: CompiledAddr, depth: u8) -> bool {
        let fst = self.0.as_fst();
        // initialize with the start node
        let mut addrs_to_visit = vec![addr];
        // iterate to specified depth
        for i in 0..depth+1 {
            // start a new vec to capture this level's addrs
            let mut level_addrs = vec![];
            while addrs_to_visit.len() > 0 {
                // pop an addr off the queue of to-visits
                let this_addr = addrs_to_visit.pop();
                // get the node at that addr
                let this_node = fst.node(this_addr);
                // for each transition that comes from this node, add its pointed-to node address
                // to this level's addrs.
                level_addrs.extend(this_node.transitions.map(|t| t.addr).collect());
            }
            // add this level's addrs to the to-visit queue
            addrs_to_visit.extend(level_addrs);
        }
        // at this point we've collected the nodes that are depth-distance from the start node.
        // check each one to see if it's a final state.
        if addrs_to_visit.len() > 0 {
            for a in addrs_to_visit {
                let this_node = fst.node(a);
                if a.is_final() {
                    return true
                }
            }
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
    use self::query::{QueryPhrase, QueryWord};

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

        let words = vec![
            vec![ QueryWord::Full{ string: String::from("100"), id: 1u64, edit_distance: 0 } ],
            vec![ QueryWord::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 0 } ],
            vec![ QueryWord::Full{ string: String::from("st"), id: 561_528u64, edit_distance: 0 } ],
        ];

        let matching_word_seq = [ &words[0][0], &words[1][0], &words[2][0] ];
        let matching_phrase = QueryPhrase::new(&matching_word_seq);
        assert_eq!(true, phrase_set.contains(matching_phrase).unwrap());

        let missing_word_seq = [ &words[0][0], &words[1][0] ];
        let missing_phrase = QueryPhrase::new(&missing_word_seq);
        assert_eq!(false, phrase_set.contains(missing_phrase).unwrap());

        let prefix = QueryWord::Prefix{ string: String::from("st"), id_range: (561_528u64, 561_531u64) };
        let has_prefix_word_seq = [ &words[0][0], &words[1][0], &prefix ];
        let has_prefix_phrase = QueryPhrase::new(&has_prefix_word_seq);
        assert!(phrase_set.contains(has_prefix_phrase).is_err());
    }

    #[test]
    fn contains_prefix_query() {
        let mut build = PhraseSetBuilder::memory();
        build.insert(&[1u64, 61_528_u64, 561_528u64]).unwrap();
        build.insert(&[61_528_u64, 561_528u64, 1u64]).unwrap();
        build.insert(&[561_528u64, 1u64, 61_528_u64]).unwrap();
        let bytes = build.into_inner().unwrap();

        let phrase_set = PhraseSet::from_bytes(bytes).unwrap();

        let words = vec![
            vec![ QueryWord::Full{ string: String::from("100"), id: 1u64, edit_distance: 0 } ],
            vec![ QueryWord::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 0 } ],
            vec![ QueryWord::Full{ string: String::from("st"), id: 561_528u64, edit_distance: 0 } ],
        ];

        let matching_word_seq = [ &words[0][0], &words[1][0] ];
        let matching_phrase = QueryPhrase::new(&matching_word_seq);
        assert_eq!(true, phrase_set.contains_prefix(matching_phrase).unwrap());

        let missing_word_seq = [ &words[0][0], &words[2][0] ];
        let missing_phrase = QueryPhrase::new(&missing_word_seq);
        assert_eq!(false, phrase_set.contains_prefix(missing_phrase).unwrap());

        let prefix = QueryWord::Prefix{ string: String::from("st"), id_range: (561_528u64, 561_531u64) };
        let has_prefix_word_seq = [ &words[0][0], &words[1][0], &prefix ];
        let has_prefix_phrase = QueryPhrase::new(&has_prefix_word_seq);
        assert!(phrase_set.contains_prefix(has_prefix_phrase).is_err());
    }

}

