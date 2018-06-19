use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::error::Error;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, BufReader, BufWriter};
use std::fs;
use std::iter;
use std::cmp::min;

use serde_json;
use fst::Streamer;

use ::prefix::{PrefixSet, PrefixSetBuilder};
use ::phrase::{PhraseSet, PhraseSetBuilder};
use ::phrase::query::{QueryPhrase, QueryWord};
use ::fuzzy::{FuzzyMap, FuzzyMapBuilder};
use regex;

pub mod unicode_ranges;
mod util;

#[derive(Default, Debug)]
pub struct FuzzyPhraseSetBuilder {
    phrases: Vec<Vec<u32>>,
    // use a btreemap for this one so we can read them out in order later
    // we'll only have one copy of each word, in the vector, so the inverse
    // map will map from a pointer to an int
    words_to_tmpids: BTreeMap<String, u32>,
    directory: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct FuzzyPhraseSetMetadata {
    index_type: String,
    format_version: u32,
    fuzzy_enabled_scripts: Vec<String>,
}

impl Default for FuzzyPhraseSetMetadata {
    fn default() -> FuzzyPhraseSetMetadata {
        FuzzyPhraseSetMetadata {
            index_type: "fuzzy_phrase_set".to_string(),
            format_version: 1,
            fuzzy_enabled_scripts: vec!["Latin".to_string(), "Greek".to_string(), "Cyrillic".to_string()],
        }
    }
}

impl FuzzyPhraseSetBuilder {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Box<Error>> {
        let directory = path.as_ref().to_owned();

        if directory.exists() {
            if !directory.is_dir() {
                return Err(Box::new(IoError::new(IoErrorKind::AlreadyExists, "File exists and is not a directory")));
            }
        } else {
            fs::create_dir(&directory)?;
        }

        Ok(FuzzyPhraseSetBuilder { directory, ..Default::default() })
    }

    pub fn insert<T: AsRef<str>>(&mut self, phrase: &[T]) -> Result<(), Box<Error>> {
        // the strategy here is to take a phrase, look at it word by word, and for any words we've
        // seen before, reuse their temp IDs, otherwise, add new words to our word map and assign them
        // new temp IDs (just autoincrementing in the order we see them) -- later once we've seen all
        // the words we'll renumber them lexicographically
        //
        // and then we're going to add the actual phrase, represented number-wise, to our phrase list

        let mut tmpid_phrase: Vec<u32> = Vec::with_capacity(phrase.len());
        for word in phrase {
            let word = word.as_ref();
            // the fact that this allocation is necessary even if the string is already in the hashmap is a bummer
            // but absent https://github.com/rust-lang/rfcs/pull/1769 , avoiding it requires a huge amount of hoop-jumping
            let string_word = word.to_string();
            let current_len = self.words_to_tmpids.len();
            let word_id = self.words_to_tmpids.entry(string_word).or_insert(current_len as u32);
            tmpid_phrase.push(word_id.to_owned());
        }
        self.phrases.push(tmpid_phrase);
        Ok(())
    }

    // convenience method that splits the input string on the space character
    // IT DOES NOT DO PROPER TOKENIZATION; if you need that, use a real tokenizer and call
    // insert directly
    pub fn insert_str(&mut self, phrase: &str) -> Result<(), Box<Error>> {
        let phrase_v: Vec<&str> = phrase.split(' ').collect();
        self.insert(&phrase_v)
    }

    pub fn finish(mut self) -> Result<(), Box<Error>> {
        // we can go from name -> tmpid
        // we need to go from tmpid -> id
        // so build a mapping that does that
        let mut tmpids_to_ids: Vec<u32> = vec![0; self.words_to_tmpids.len()];

        let prefix_writer = BufWriter::new(fs::File::create(self.directory.join(Path::new("prefix.fst")))?);
        let mut prefix_set_builder = PrefixSetBuilder::new(prefix_writer)?;

        let mut fuzzy_map_builder = FuzzyMapBuilder::new(self.directory.join(Path::new("fuzzy")), 1)?;

        let metadata = FuzzyPhraseSetMetadata::default();

        // this is a regex set to decide whether to index somehing for fuzzy matching
        let allowed_scripts = &metadata.fuzzy_enabled_scripts.iter().map(
            |s| unicode_ranges::get_script_by_name(s)
        ).collect::<Option<Vec<_>>>().ok_or("unknown script")?;
        let script_regex = regex::Regex::new(
            &unicode_ranges::get_pattern_for_scripts(&allowed_scripts),
        ).unwrap();

        // words_to_tmpids is a btreemap over word keys,
        // so when we iterate over it, we'll get back words sorted
        // we'll do three things with that:
        // - build up our prefix set
        // - map from temporary IDs to lex ids (which we can get just be enumerating our sorted list)
        // - build up our fuzzy set (this one doesn't require the sorted words, but it doesn't hurt)
        for (id, (word, tmpid)) in self.words_to_tmpids.iter().enumerate() {
            let id = id as u32;

            prefix_set_builder.insert(word)?;

            let allowed = util::can_fuzzy_match(word, &script_regex);

            if allowed {
                fuzzy_map_builder.insert(word, id);
            }

            tmpids_to_ids[*tmpid as usize] = id;
        }

        prefix_set_builder.finish()?;
        fuzzy_map_builder.finish()?;

        // next, renumber all of the current phrases with real rather than temp IDs
        for phrase in self.phrases.iter_mut() {
            for word_idx in (*phrase).iter_mut() {
                *word_idx = tmpids_to_ids[*word_idx as usize];
            }
        }

        self.phrases.sort();

        let phrase_writer = BufWriter::new(fs::File::create(self.directory.join(Path::new("phrase.fst")))?);
        let mut phrase_set_builder = PhraseSetBuilder::new(phrase_writer)?;

        for phrase in self.phrases {
            phrase_set_builder.insert(&phrase)?;
        }

        phrase_set_builder.finish()?;

        let metadata_writer = BufWriter::new(fs::File::create(self.directory.join(Path::new("metadata.json")))?);
        serde_json::to_writer_pretty(metadata_writer, &metadata)?;

        Ok(())
    }
}

pub struct FuzzyPhraseSet {
    prefix_set: PrefixSet,
    phrase_set: PhraseSet,
    fuzzy_map: FuzzyMap,
    word_list: Vec<String>,
    script_regex: regex::Regex,
}

#[derive(Debug, Eq, PartialEq)]
pub struct FuzzyMatchResult {
    phrase: Vec<String>,
    edit_distance: u8,
}

#[derive(Debug, Eq, PartialEq)]
pub struct FuzzyWindowResult {
    phrase: Vec<String>,
    edit_distance: u8,
    start_position: usize,
    ends_in_prefix: bool,
}

impl FuzzyPhraseSet {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Box<Error>> {
        // the path of a fuzzy phrase set is a directory that has all the subcomponents in it at predictable URLs
        // the prefix graph and phrase graph are each single-file FSTs; the fuzzy graph is multiple files so we
        // pass in a their shared prefix to the fuzzy graph constructor
        // we also have a config file that has version info (with metadata about the index contents to come)
        let directory = path.as_ref();

        if !directory.exists() || !directory.is_dir() {
            return Err(Box::new(IoError::new(IoErrorKind::NotFound, "File does not exist or is not a directory")));
        }

        let metadata_reader = BufReader::new(fs::File::open(directory.join(Path::new("metadata.json")))?);
        let metadata: FuzzyPhraseSetMetadata = serde_json::from_reader(metadata_reader)?;
        if metadata != FuzzyPhraseSetMetadata::default() {
            return Err(Box::new(IoError::new(IoErrorKind::InvalidData, "Unexpected structure metadata")));
        }

        let allowed_scripts = &metadata.fuzzy_enabled_scripts.iter().map(
            |s| unicode_ranges::get_script_by_name(s)
        ).collect::<Option<Vec<_>>>().ok_or("unknown script")?;
        let script_regex = regex::Regex::new(
            &unicode_ranges::get_pattern_for_scripts(&allowed_scripts),
        ).unwrap();

        let prefix_path = directory.join(Path::new("prefix.fst"));
        if !prefix_path.exists() {
            return Err(Box::new(IoError::new(IoErrorKind::NotFound, "Prefix FST does not exist")));
        }
        let prefix_set = unsafe { PrefixSet::from_path(&prefix_path) }?;

        // the fuzzy graph needs to be able to go from ID to actual word
        // one idea was to look this up from the prefix graph, which can do backwards lookups
        // (id to string), but this turned out to be too slow, so instead we'll just hold
        // an array of all the words in memory, which turns out to be small enough to just do.
        // we can get this by iterating over the prefix graph contents and exploding them into a vector
        let mut word_list = Vec::<String>::with_capacity(prefix_set.len());
        {
            let mut stream = prefix_set.stream();
            while let Some((word, _id)) = stream.next() {
                word_list.push(String::from_utf8(word.to_owned())?);
            }
        }

        let phrase_path = directory.join(Path::new("phrase.fst"));
        if !phrase_path.exists() {
            return Err(Box::new(IoError::new(IoErrorKind::NotFound, "Phrase FST does not exist")));
        }
        let phrase_set = unsafe { PhraseSet::from_path(&phrase_path) }?;

        let fuzzy_path = directory.join(Path::new("fuzzy"));
        let fuzzy_map = unsafe { FuzzyMap::from_path(&fuzzy_path) }?;

        Ok(FuzzyPhraseSet { prefix_set, phrase_set, fuzzy_map, word_list, script_regex })
    }

    pub fn can_fuzzy_match(&self, word: &str) -> bool {
        util::can_fuzzy_match(word, &self.script_regex)
    }

    pub fn contains<T: AsRef<str>>(&self, phrase: &[T]) -> Result<bool, Box<Error>> {
        // strategy: get each word's ID from the prefix graph (or return false if any are missing)
        // and then look up that ID sequence in the phrase graph
        let mut id_phrase: Vec<QueryWord> = Vec::with_capacity(phrase.len());
        for word in phrase {
            match self.prefix_set.get(word.as_ref()) {
                Some(word_id) => { id_phrase.push(QueryWord::new_full(word_id as u32, 0)) },
                None => { return Ok(false) }
            }
        }
        Ok(self.phrase_set.contains(QueryPhrase::new(&id_phrase)?)?)
    }

    // convenience method that splits the input string on the space character
    // IT DOES NOT DO PROPER TOKENIZATION; if you need that, use a real tokenizer and call
    // contains directly
    pub fn contains_str(&self, phrase: &str) -> Result<bool, Box<Error>> {
        let phrase_v: Vec<&str> = phrase.split(' ').collect();
        self.contains(&phrase_v)
    }

    pub fn contains_prefix<T: AsRef<str>>(&self, phrase: &[T]) -> Result<bool, Box<Error>> {
        // strategy: get each word's ID from the prefix graph (or return false if any are missing)
        // except for the last one; do a word prefix lookup instead and construct a prefix range
        // and then look up that sequence with a prefix lookup in the phrase graph
        let mut id_phrase: Vec<QueryWord> = Vec::with_capacity(phrase.len());
        if phrase.len() > 0 {
            let last_idx = phrase.len() - 1;
            for word in phrase[..last_idx].iter() {
                match self.prefix_set.get(word.as_ref()) {
                    Some(word_id) => { id_phrase.push(QueryWord::new_full(word_id as u32, 0)) },
                    None => { return Ok(false) }
                }
            }
            match self.prefix_set.get_prefix_range(phrase[last_idx].as_ref()) {
                Some((word_id_start, word_id_end)) => { id_phrase.push(QueryWord::new_prefix((word_id_start.value() as u32, word_id_end.value() as u32))) },
                None => { return Ok(false) }
            }
        }
        Ok(self.phrase_set.contains_prefix(QueryPhrase::new(&id_phrase)?)?)
    }

    // convenience method that splits the input string on the space character
    // IT DOES NOT DO PROPER TOKENIZATION; if you need that, use a real tokenizer and call
    // contains_prefix directly
    pub fn contains_prefix_str(&self, phrase: &str) -> Result<bool, Box<Error>> {
        let phrase_v: Vec<&str> = phrase.split(' ').collect();
        self.contains_prefix(&phrase_v)
    }

    #[inline(always)]
    fn get_nonterminal_word_possibilities(&self, word: &str, edit_distance: u8) -> Result<Option<Vec<QueryWord>>, Box<Error>> {
        if self.can_fuzzy_match(word) {
            let fuzzy_results = self.fuzzy_map.lookup(&word, edit_distance, |id| &self.word_list[id as usize])?;
            if fuzzy_results.len() == 0 {
                Ok(None)
            } else {
                let mut variants: Vec<QueryWord> = Vec::with_capacity(fuzzy_results.len());
                for result in fuzzy_results {
                    variants.push(QueryWord::new_full(result.id, result.edit_distance));
                }
                Ok(Some(variants))
            }
        } else {
            match self.prefix_set.get(&word) {
                Some(word_id) => { Ok(Some(vec![QueryWord::new_full(word_id as u32, 0)])) },
                None => { Ok(None) }
            }
        }
    }

    #[inline(always)]
    fn get_terminal_word_possibilities(&self, word: &str, edit_distance: u8) -> Result<Option<Vec<QueryWord>>, Box<Error>> {
        // last word: try both prefix and, if eligible, fuzzy lookup, and return nothing if both fail
        let mut last_variants: Vec<QueryWord> = Vec::new();
        if let Some((word_id_start, word_id_end)) = self.prefix_set.get_prefix_range(word) {
            last_variants.push(QueryWord::new_prefix((word_id_start.value() as u32, word_id_end.value() as u32)));
        }
        if self.can_fuzzy_match(word) {
            let last_fuzzy_results = self.fuzzy_map.lookup(word, edit_distance, |id| &self.word_list[id as usize])?;
            for result in last_fuzzy_results {
                last_variants.push(QueryWord::new_full(result.id, result.edit_distance));
            }
        }
        if last_variants.len() > 0 {
            Ok(Some(last_variants))
        } else {
            Ok(None)
        }
    }

    pub fn fuzzy_match<T: AsRef<str>>(&self, phrase: &[T], max_word_dist: u8, max_phrase_dist: u8) -> Result<Vec<FuzzyMatchResult>, Box<Error>> {
        // strategy: look up each word in the fuzzy graph
        // and then construct a vector of vectors representing all the word variants that could reside in each slot
        // in the phrase, and then recursively enumerate every combination of variants and look them each up in the phrase graph

        let mut word_possibilities: Vec<Vec<QueryWord>> = Vec::with_capacity(phrase.len());

        // later we should preserve the max edit distance we can support with the structure we have built
        // and either throw an error or silently constrain to that
        // but for now, we're hard-coded to one at build time, so hard coded to one and read time
        let edit_distance = min(max_word_dist, 1);

        // the map is executed lazily, so we can early-bail without correcting everything
        for matches in phrase.iter().map(|word| self.get_nonterminal_word_possibilities(word.as_ref(), edit_distance)) {
            match matches? {
                Some(possibilities) => word_possibilities.push(possibilities),
                None => return Ok(Vec::new()),
            }
        }

        let phrase_matches = self.phrase_set.recursive_match_combinations(word_possibilities, max_phrase_dist)?;

        let mut results: Vec<FuzzyMatchResult> = Vec::new();
        for phrase_p in &phrase_matches {
            results.push(FuzzyMatchResult {
                phrase: phrase_p.iter().map(|qw| match qw {
                    QueryWord::Full { id, .. } => self.word_list[*id as usize].clone(),
                    _ => panic!("prefixes not allowed"),
                }).collect::<Vec<String>>(),
                edit_distance: phrase_p.iter().map(|qw| match qw {
                    QueryWord::Full { edit_distance, .. } => *edit_distance,
                    _ => panic!("prefixes not allowed"),
                }).sum(),
            });
        }

        Ok(results)
    }

    pub fn fuzzy_match_str(&self, phrase: &str, max_word_dist: u8, max_phrase_dist: u8) -> Result<Vec<FuzzyMatchResult>, Box<Error>> {
        let phrase_v: Vec<&str> = phrase.split(' ').collect();
        self.fuzzy_match(&phrase_v, max_word_dist, max_phrase_dist)
    }

    pub fn fuzzy_match_prefix<T: AsRef<str>>(&self, phrase: &[T], max_word_dist: u8, max_phrase_dist: u8) -> Result<Vec<FuzzyMatchResult>, Box<Error>> {
        // strategy: look up each word in the fuzzy graph, and also look up the last one in the prefix graph
        // and then construct a vector of vectors representing all the word variants that could reside in each slot
        // in the phrase, and then recursively enumerate every combination of variants and look them each up in the phrase graph

        let mut word_possibilities: Vec<Vec<QueryWord>> = Vec::with_capacity(phrase.len());

        if phrase.len() == 0 {
            return Ok(Vec::new());
        }

        // later we should preserve the max edit distance we can support with the structure we have built
        // and either throw an error or silently constrain to that
        // but for now, we're hard-coded to one at build time, so hard coded to one and read time
        let edit_distance = min(max_word_dist, 1);

        // all words but the last one: fuzzy-lookup if eligible, or exact-match if not,
        // and return nothing if those fail
        let last_idx = phrase.len() - 1;
        for matches in phrase[..last_idx].iter().map(|word| self.get_nonterminal_word_possibilities(word.as_ref(), edit_distance)) {
            match matches? {
                Some(possibilities) => word_possibilities.push(possibilities),
                None => return Ok(Vec::new()),
            }
        }
        match self.get_terminal_word_possibilities(phrase[last_idx].as_ref(), edit_distance)? {
            Some(possibilities) => word_possibilities.push(possibilities),
            None => return Ok(Vec::new()),
        }

        let phrase_matches = self.phrase_set.recursive_match_combinations_as_prefixes(word_possibilities, max_phrase_dist)?;

        let mut results: Vec<FuzzyMatchResult> = Vec::new();
        for phrase_p in &phrase_matches {
            results.push(FuzzyMatchResult {
                phrase: phrase_p.iter().enumerate().map(|(i, qw)| match qw {
                    QueryWord::Full { id, .. } => self.word_list[*id as usize].clone(),
                    QueryWord::Prefix { .. } => phrase[i].as_ref().to_owned(),
                }).collect::<Vec<String>>(),
                edit_distance: phrase_p.iter().map(|qw| match qw {
                    QueryWord::Full { edit_distance, .. } => *edit_distance,
                    QueryWord::Prefix { .. } => 0u8,
                }).sum(),
            })
        }

        Ok(results)
    }

    pub fn fuzzy_match_prefix_str(&self, phrase: &str, max_word_dist: u8, max_phrase_dist: u8) -> Result<Vec<FuzzyMatchResult>, Box<Error>> {
        let phrase_v: Vec<&str> = phrase.split(' ').collect();
        self.fuzzy_match_prefix(&phrase_v, max_word_dist, max_phrase_dist)
    }

    pub fn fuzzy_match_windows<T: AsRef<str>>(&self, phrase: &[T], max_word_dist: u8, max_phrase_dist: u8, ends_in_prefix: bool) -> Result<Vec<FuzzyWindowResult>, Box<Error>> {
        // this is a little different than the regular fuzzy match in that we're considering multiple possible substrings
        // we'll start by trying to fuzzy-match all the words, but some of those will likely fail -- rather than early-returning
        // like in regular fuzzy match, we'll keep going but those failed words will effectively wall off possible matching
        // subphrases from eachother, so we'll end up with multiple candidate subphrases to explore.
        // (hence the extra nesting -- a list of word sequences, each sequence being a list of word slots, each slot being a
        // list of fuzzy-match variants)

        if phrase.len() == 0 {
            return Ok(Vec::new());
        }

        #[derive(Debug)]
        struct Subquery {
            start_position: usize,
            ends_in_prefix: bool,
            word_possibilities: Vec<Vec<QueryWord>>
        }
        let mut subqueries: Vec<Subquery> = Vec::new();

        let edit_distance = min(max_word_dist, 1);

        let seq: Box<Iterator<Item=Result<Option<Vec<QueryWord>>, Box<Error>>>> = if ends_in_prefix {
            let last_idx = phrase.len() - 1;
            let i = phrase[..last_idx].iter().map(
                |word| self.get_nonterminal_word_possibilities(word.as_ref(), edit_distance)
            ).chain(iter::once(last_idx).map(
                |idx| self.get_terminal_word_possibilities(phrase[idx].as_ref(), edit_distance))
            );
            Box::new(i)
        } else {
            let i = phrase.iter().map(|word| self.get_nonterminal_word_possibilities(word.as_ref(), edit_distance));
            Box::new(i)
        };

        let mut sq: Subquery = Subquery { start_position: 0, ends_in_prefix: false, word_possibilities: Vec::new() };
        for (i, matches) in seq.chain(iter::once(Ok(None))).enumerate() {
            match matches.unwrap() {
                Some(p) => {
                    sq.word_possibilities.push(p);
                    if sq.word_possibilities.len() == 1 {
                        // this was the first thing to be added to this list
                        sq.start_position = i;
                    }
                }
                None => {
                    if sq.word_possibilities.len() > 0 {
                        // there's something to do
                        if i == phrase.len() && ends_in_prefix {
                            sq.ends_in_prefix = true;
                        }
                        subqueries.push(sq);
                        sq = Subquery { start_position: 0, ends_in_prefix: false, word_possibilities: Vec::new() };
                    }
                },
            }
        }

        // the things we're looking for will lie entirely within one of our identified chunks of
        // contiguous matched words, but could start on any of said words (they'll end, at latest,
        // and the end of the chunk), so, iterate over the chunks and then iterate over the
        // possible start words
        let mut results: Vec<FuzzyWindowResult> = Vec::new();
        for chunk in subqueries.iter() {
            for i in 0..chunk.word_possibilities.len() {
                let mut phrase_matches = self.phrase_set.recursive_match_combinations_as_windows(
                    chunk.word_possibilities[i..].iter().cloned().collect(),
                    max_phrase_dist,
                    chunk.ends_in_prefix
                )?;
                for (phrase_p, sq_ends_in_prefix) in &phrase_matches {
                    results.push(FuzzyWindowResult {
                        phrase: phrase_p.iter().enumerate().map(|(i, qw)| match qw {
                            QueryWord::Full { id, .. } => self.word_list[*id as usize].clone(),
                            QueryWord::Prefix { .. } => phrase[chunk.start_position + i].as_ref().to_owned(),
                        }).collect::<Vec<String>>(),
                        edit_distance: phrase_p.iter().map(|qw| match qw {
                            QueryWord::Full { edit_distance, .. } => *edit_distance,
                            QueryWord::Prefix { .. } => 0u8,
                        }).sum(),
                        start_position: chunk.start_position + i,
                        ends_in_prefix: *sq_ends_in_prefix,
                    })
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    extern crate tempfile;
    extern crate lazy_static;

    use super::*;

    lazy_static! {
        static ref DIR: tempfile::TempDir = tempfile::tempdir().unwrap();
        static ref SET: FuzzyPhraseSet = {
            let mut builder = FuzzyPhraseSetBuilder::new(&DIR.path()).unwrap();
            builder.insert_str("100 main street").unwrap();
            builder.insert_str("200 main street").unwrap();
            builder.insert_str("100 main ave").unwrap();
            builder.insert_str("300 mlk blvd").unwrap();
            builder.finish().unwrap();

            FuzzyPhraseSet::from_path(&DIR.path()).unwrap()
        };
    }

    #[test]
    fn glue_build() -> () {
        lazy_static::initialize(&SET);
    }

    #[test]
    fn glue_contains() -> () {
        // contains
        assert!(SET.contains_str("100 main street").unwrap());
        assert!(SET.contains_str("200 main street").unwrap());
        assert!(SET.contains_str("100 main ave").unwrap());
        assert!(SET.contains_str("300 mlk blvd").unwrap());
    }

    #[test]
    fn glue_test_asref() -> () {
        // test that we're generic over vectors and arrays, and also Strings and strs
        // testing this for contains because it's simplest, but we reuse this pattern elsewhere,
        // e.g., for insert
        assert!(SET.contains_str("100 main street").unwrap());
        let phrase_static = ["100", "main", "street"];
        assert!(SET.contains(&phrase_static).unwrap());
        let phrase_vec: Vec<String> = vec!["100".to_string(), "main".to_string(), "street".to_string()];
        assert!(SET.contains(&phrase_vec).unwrap());
        let ref_phrase_vec: Vec<&str> = phrase_vec.iter().map(|s| s.as_str()).collect();
        assert!(SET.contains(&ref_phrase_vec).unwrap());
    }

    #[test]
    fn glue_doesnt_contain() -> () {
        assert!(!SET.contains_str("x").unwrap());
        assert!(!SET.contains_str("100 main").unwrap());
        assert!(!SET.contains_str("100 main s").unwrap());
        assert!(!SET.contains_str("100 main streetr").unwrap());
        assert!(!SET.contains_str("100 main street r").unwrap());
        assert!(!SET.contains_str("100 main street ave").unwrap());
    }

    #[test]
    fn glue_contains_prefix_exact() -> () {
        // contains prefix -- everything that works in full works as prefix
        assert!(SET.contains_prefix_str("100 main street").unwrap());
        assert!(SET.contains_prefix_str("200 main street").unwrap());
        assert!(SET.contains_prefix_str("100 main ave").unwrap());
        assert!(SET.contains_prefix_str("300 mlk blvd").unwrap());
    }

    #[test]
    fn glue_contains_prefix_partial_word() -> () {
        // contains prefix -- drop a letter
        assert!(SET.contains_prefix_str("100 main stree").unwrap());
        assert!(SET.contains_prefix_str("200 main stree").unwrap());
        assert!(SET.contains_prefix_str("100 main av").unwrap());
        assert!(SET.contains_prefix_str("300 mlk blv").unwrap());
    }

    #[test]
    fn glue_contains_prefix_dropped_word() -> () {
        // contains prefix -- drop a word
        assert!(SET.contains_prefix_str("100 main").unwrap());
        assert!(SET.contains_prefix_str("200 main").unwrap());
        assert!(SET.contains_prefix_str("100 main").unwrap());
        assert!(SET.contains_prefix_str("300 mlk").unwrap());
    }

    #[test]
    fn glue_doesnt_contain_prefix() -> () {
        // contains prefix -- drop a word
        assert!(!SET.contains_prefix_str("100 man").unwrap());
        assert!(!SET.contains_prefix_str("400 main").unwrap());
        assert!(!SET.contains_prefix_str("100 main street x").unwrap());
    }

    #[test]
    fn glue_fuzzy_match() -> () {
        assert_eq!(
            SET.fuzzy_match(&["100", "man", "street"], 1, 1).unwrap(),
            vec![
                FuzzyMatchResult { phrase: vec!["100".to_string(), "main".to_string(), "street".to_string()], edit_distance: 1 },
            ]
        );

        assert_eq!(
            SET.fuzzy_match(&["100", "man", "stret"], 1, 2).unwrap(),
            vec![
                FuzzyMatchResult { phrase: vec!["100".to_string(), "main".to_string(), "street".to_string()], edit_distance: 2 },
            ]
        );
    }

    #[test]
    fn glue_fuzzy_match_prefix() -> () {
        assert_eq!(
            SET.fuzzy_match_prefix(&["100", "man"], 1, 1).unwrap(),
            vec![
                FuzzyMatchResult { phrase: vec!["100".to_string(), "main".to_string()], edit_distance: 1 },
            ]
        );

        assert_eq!(
            SET.fuzzy_match_prefix(&["100", "man", "str"], 1, 1).unwrap(),
            vec![
                FuzzyMatchResult { phrase: vec!["100".to_string(), "main".to_string(), "str".to_string()], edit_distance: 1 },
            ]
        );
    }

    #[test]
    fn glue_fuzzy_match_windows() -> () {
        assert_eq!(
            SET.fuzzy_match_windows(&["100", "main", "street", "washington", "300"], 1, 1, true).unwrap(),
            vec![
                FuzzyWindowResult { phrase: vec!["100".to_string(), "main".to_string(), "street".to_string()], edit_distance: 0, start_position: 0, ends_in_prefix: false },
                FuzzyWindowResult { phrase: vec!["300".to_string()], edit_distance: 0, start_position: 4, ends_in_prefix: true }
            ]
        );

        assert_eq!(
            SET.fuzzy_match_windows(&["100", "main", "street", "washington", "300"], 1, 1, false).unwrap(),
            vec![
                FuzzyWindowResult { phrase: vec!["100".to_string(), "main".to_string(), "street".to_string()], edit_distance: 0, start_position: 0, ends_in_prefix: false },
            ]
        );
    }
}

#[cfg(test)] mod fuzz_tests;