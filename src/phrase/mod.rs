pub mod util;
pub mod query;

use std::io;
#[cfg(feature = "mmap")]
use std::path::Path;

use fst;
use fst::{IntoStreamer, Set, SetBuilder};
use fst::raw::{Node, Transition, CompiledAddr};

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
    pub fn contains(&self, phrase: QueryPhrase) -> bool {
        if phrase.has_prefix {
            // TODO if last word is prefix, do some special shit
            false
        } else {
            // if all words are Full
            // construct key and perform typical contains query
            let key = phrase.full_word_key();
            self.0.contains(key)
        }
    }

    fn partial_search(&self, start_addr: CompiledAddr, key: &[u8]) -> Option<CompiledAddr> {
        let fst = self.0.as_fst();
        let mut node = fst.node(start_addr);
        // move through the tree byte by byte
        for b in key {
            node = match node.find_input(b) {
                None => return None,
                Some(i) => fst.node(node.transition_addr(i)),
            }
        }
        return Some(node.addr())
    }

    // TODO some special shit
    fn contains_prefix(&self, phrase: QueryPhrase) -> bool {
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
        match self.partial_search(full_word_addr, prefix_min_key) {
            Some(addr) => {
                let prefix_min_node = fst.node(addr);
                if prefix_min_node.is_final() {
                    return true
                }
            },
            _ => (),
        }
        match self.partial_search(full_word_addr, prefix_max_key) {
            Some(addr) => {
                let prefix_min_node = fst.node(addr);
                if prefix_min_node.is_final() {
                    return true
                }
            },
            _ => (),
        }

        let mut lo_bound = node.addr();
        let mut hi_bound = node.addr();
		// isolate the subtree
        for i in 0..3 {
            let mut middle: HashSet::new();
            if lo_bound != None {
                let lo_bound_node = fst.node(lo_bound);
                let lo_bound = match lo_bound_node.find_input(prefix_min_key[i]) {
                    None => None,
                    Some(a) => lo_bound_node.transition_addr(a),
                };
                for t in lo_bound_node.transitions().filter(|t| t.inp > prefix_min_key[i]) {
                    middle.insert(t.addr);
                }
            }
            if hi_bound != None {
                let hi_bound_node = fst.node(hi_bound);
                let hi_bound = match hi_bound_node.find_input(prefix_max_key[i]) {
                    None => None,
                    Some(a) => fst.node(hi_bound.transition_addr(a)),
                };
                for t in hi_bound_node.transitions().filter(|t| t.inp < prefix_max_key[i]) {
                    middle.insert(t.addr);
                }
            }

            // for all middle nodes:
            let depth = 2 - i;
            for m in &middle {
                match self.final_at_depth(m, depth) {
                    true => return true,
                    false => (),
                }
            }
        }

        return true
    }

    fn final_at_depth(&self, addr: CompiledAddr, depth: u8) -> bool {
        let fst = self.0.as_fst();
        let mut addrs_to_visit = vec![addr];
        for i in 0..depth {
            let mut level_addrs = vec![];
            while addrs_to_visit.len() > 0 {
                let this_addr = addrs_to_visit.pop();
                let this_node = fst.node(this_addr);
                level_addrs.extend(this_node.transitions.map(|t| t.addr).collect());
            }
            addrs_to_visit.extend(level_addrs);
        }
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
        assert_eq!(true, phrase_set.contains(matching_phrase));

        let missing_word_seq = [ &words[0][0], &words[1][0] ];
        let missing_phrase = QueryPhrase::new(&missing_word_seq);
        assert_eq!(false, phrase_set.contains(missing_phrase));
    }
}

