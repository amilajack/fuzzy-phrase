use fst::{IntoStreamer, Streamer, Automaton};
use std::fs;
use std::error::Error;
use std::cmp::{min, Ordering};
use itertools::Itertools;
use fst::raw;
use fst::Error as FstError;
use fst::automaton::{AlwaysMatch};
#[cfg(feature = "mmap")]
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use serde::{Deserialize, Serialize};
use rmps::{Deserializer, Serializer};
#[cfg(test)] extern crate reqwest;

use fuzzy::util::multi_modified_damlev_hint;

static MULTI_FLAG: u64 = 1 << 63;
static MULTI_MASK: u64 = !(1 << 63);

pub struct FuzzyMap {
    id_list: Vec<Vec<u32>>,
    fst: raw::Fst
}

#[derive(Serialize, Deserialize)]
pub struct SerializableIdList(Vec<Vec<u32>>);

#[derive(PartialEq, Eq, Debug)]
pub struct FuzzyMapLookupResult {
    pub word: String,
    pub id: u32,
    pub edit_distance: u8,
}

impl Ord for FuzzyMapLookupResult {
    fn cmp(&self, other: &FuzzyMapLookupResult) -> Ordering {
        (self.edit_distance, self.id, &self.word).cmp(&(other.edit_distance, other.id, &other.word))
    }
}

impl PartialOrd for FuzzyMapLookupResult {
    fn partial_cmp(&self, other: &FuzzyMapLookupResult) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FuzzyMap {
    #[cfg(feature = "mmap")]
    pub unsafe fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, FstError> {
        let file_start = path.as_ref();
        let fst = raw::Fst::from_path(file_start.with_extension(".fst")).unwrap();
        let mf_reader = BufReader::new(fs::File::open(file_start.with_extension(".msg"))?);
        let id_list: SerializableIdList = Deserialize::deserialize(&mut Deserializer::new(mf_reader)).unwrap();
        Ok(FuzzyMap { id_list: id_list.0, fst: fst })
    }

    pub fn contains<K: AsRef<[u8]>>(&self, key: K) -> bool {
        self.fst.contains_key(key)
    }

    pub fn stream(&self) -> Stream {
        Stream(self.fst.stream())
    }

    pub fn range(&self) -> StreamBuilder {
        StreamBuilder(self.fst.range())
    }

    pub fn search<A: Automaton>(&self, aut: A) -> StreamBuilder<A> {
        StreamBuilder(self.fst.search(aut))
    }

    pub fn len(&self) -> usize {
        self.fst.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fst.is_empty()
    }

    pub fn as_fst(&self) -> &raw::Fst {
        &self.fst
    }

    // this one is from Map
    pub fn get<K: AsRef<[u8]>>(&self, key: K) -> Option<u64> {
        self.fst.get(key).map(|output| output.value())
    }

    fn find_matching_variants(&self, query: &[u8], indices: &[usize], position: usize, edit_distance: usize, node: &raw::Node, so_far: u64, out: &mut Vec<u64>) {
        if (indices.len() - 1 - position) <= edit_distance {
            // we're to the end of our string or within the edit distance
            // so if we're on a final string, emit output
            if node.is_final() {
                out.push(so_far + node.final_output().value());
            }
        }

        for i in position..min(position + edit_distance + 1, indices.len() - 1) {
            let mut found = true;
            let mut search_node = node.to_owned();
            let mut search_output = 0;
            for byte in &query[indices[i]..indices[i+1]] {
                if let Some(x) = search_node.find_input(*byte) {
                    let trans = search_node.transition(x);
                    search_output += trans.out.value();
                    search_node = self.fst.node(trans.addr);
                } else {
                    found = false;
                    break;
                }
            }
            if found {
                self.find_matching_variants(query, indices, i + 1, edit_distance - (i - position), &search_node, so_far + search_output, out);
            }
        }
    }

    fn find_matching_variants_ascii(&self, query: &[u8], position: usize, edit_distance: usize, node: &raw::Node, so_far: u64, out: &mut Vec<u64>) {
        if (query.len() - position) <= edit_distance {
            // we're to the end of our string or within the edit distance
            // so if we're on a final string, emit output
            if node.is_final() {
                out.push(so_far + node.final_output().value());
            }
        }

        for i in position..min(position + edit_distance + 1, query.len()) {
            if let Some(x) = node.find_input(query[i]) {
                let trans = node.transition(x);
                self.find_matching_variants_ascii(query, i + 1, edit_distance - (i - position), &self.fst.node(trans.addr), so_far + trans.out.value(), out);
            }
        }
    }

    pub fn lookup<'a, F>(&self, query: &str, edit_distance: u8, lookup_fn: F) -> Result<Vec<FuzzyMapLookupResult>, Box<Error>> where F: Fn(u32) -> &'a str {
        let mut matches = Vec::<u32>::new();

        let mut variant_ids: Vec<u64> = Vec::new();
        if query.is_ascii() {
            self.find_matching_variants_ascii(query.as_bytes(), 0, edit_distance as usize, &self.fst.root(), 0, &mut variant_ids);
        } else {
            let mut query_indices = query.char_indices().map(|(i, _c)| i).collect::<Vec<_>>();
            query_indices.push(query.len());
            self.find_matching_variants(query.as_bytes(), &query_indices, 0, edit_distance as usize, &self.fst.root(), 0, &mut variant_ids);
        }

        // check the query itself and the variants
        for uidx in variant_ids {
            if uidx & MULTI_FLAG != 0 {
                for x in &(self.id_list)[(uidx & MULTI_MASK) as usize] {
                    matches.push(*x as u32);
                }
            } else {
                matches.push(uidx as u32);
            }
        }
        //return all ids that match
        matches.sort();
        matches.dedup();

        let match_words = matches.iter().map(|id| lookup_fn(*id)).collect::<Vec<_>>();
        let distances = multi_modified_damlev_hint(query, &match_words, edit_distance as u32);

        let mut out = matches
            .into_iter()
            .enumerate()
            .filter_map(|(i, id)| {
                if distances[i] <= edit_distance as u32 {
                    Some(FuzzyMapLookupResult { word: match_words[i].to_owned(), id: id as u32, edit_distance: distances[i] as u8 })
                } else {
                    None
                }
            })
            .collect::<Vec<FuzzyMapLookupResult>>();
        out.sort();
        Ok(out)
    }
}

pub struct FuzzyMapBuilder {
    id_builder: Vec<Vec<u32>>,
    builder: raw::Builder<BufWriter<File>>,
    file_path: PathBuf,
    word_variants: Vec<(String, u32)>,
    edit_distance: u8,
}

impl FuzzyMapBuilder {
    pub fn new<P: AsRef<Path>>(path: P, edit_distance: u8) -> Result<Self, Box<Error>> {
        let file_start = path.as_ref().to_owned();
        let fst_wtr = BufWriter::new(fs::File::create(file_start.with_extension(".fst"))?);

        Ok(FuzzyMapBuilder {
            builder: raw::Builder::new_type(fst_wtr, 0)?,
            id_builder: Vec::<Vec<u32>>::new(),
            file_path: file_start,
            word_variants: Vec::<(String, u32)>::new(),
            edit_distance: edit_distance,
        })
    }

    pub fn build_from_iter<'a, T, P: AsRef<Path>>(path: P, words: T, edit_distance: u8) -> Result<(), Box<Error>> where T: Iterator<Item=&'a str> {
        let mut fuzzy_map_builder = FuzzyMapBuilder::new(path, edit_distance)?;

        for (i, word) in words.enumerate() {
            fuzzy_map_builder.insert(word, i as u32);
        }
        fuzzy_map_builder.finish()?;
        Ok(())
    }

    pub fn insert(&mut self, key: &str, id: u32) -> () {
        self.word_variants.push((key.to_owned(), id));
        let variants = super::get_variants(&key, self.edit_distance);
        for j in variants.into_iter() {
            self.word_variants.push((j, id));
        }
    }

    pub fn extend_iter<'a, T, I>(&mut self, iter: I) -> Result<(), FstError> where T: AsRef<[u8]>, I: IntoIterator<Item=&'a str> {
        for (i, word) in iter.into_iter().enumerate() {
            self.insert(word, i as u32);
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), FstError> {
        self.word_variants.sort();

        for (key, group) in &(&self.word_variants).iter().dedup().group_by(|t| &t.0) {
            let opts = group.collect::<Vec<_>>();
            let id = if opts.len() == 1 {
                opts[0].1 as u64
            } else {
                self.id_builder.push((&opts).iter().map(|t| t.1).collect::<Vec<_>>());
                (self.id_builder.len() - 1) as u64 | MULTI_FLAG
            };
            self.builder.insert(key, id)?;
        }
        let mf_wtr = BufWriter::new(fs::File::create(self.file_path.with_extension(".msg"))?);
        SerializableIdList(self.id_builder).serialize(&mut Serializer::new(mf_wtr)).unwrap();
        self.builder.finish()
    }

    pub fn bytes_written(&self) -> u64 {
        self.builder.bytes_written()
    }
}

pub struct Stream<'s, A=AlwaysMatch>(raw::Stream<'s, A>) where A: Automaton;

impl<'s, A: Automaton> Stream<'s, A> {
    #[doc(hidden)]
    pub fn new(fst_stream: raw::Stream<'s, A>) -> Self {
        Stream(fst_stream)
    }

    pub fn into_byte_vec(self) -> Vec<(Vec<u8>, u64)> {
        self.0.into_byte_vec()
    }

    pub fn into_str_vec(self) -> Result<Vec<(String, u64)>, FstError> {
        self.0.into_str_vec()
    }

    pub fn into_strs(self) -> Result<Vec<String>, FstError> {
        self.0.into_str_keys()
    }

    pub fn into_bytes(self) -> Vec<Vec<u8>> {
        self.0.into_byte_keys()
    }
}

impl<'a, 's, A: Automaton> Streamer<'a> for Stream<'s, A> {
    type Item = (&'a [u8], u64);

    fn next(&'a mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, out)| (key, out.value()))
    }
}

pub struct StreamBuilder<'s, A=AlwaysMatch>(raw::StreamBuilder<'s, A>);

impl<'s, A: Automaton> StreamBuilder<'s, A> {
    pub fn ge<T: AsRef<[u8]>>(self, bound: T) -> Self {
        StreamBuilder(self.0.ge(bound))
    }

    pub fn gt<T: AsRef<[u8]>>(self, bound: T) -> Self {
        StreamBuilder(self.0.gt(bound))
    }

    pub fn le<T: AsRef<[u8]>>(self, bound: T) -> Self {
        StreamBuilder(self.0.le(bound))
    }

    pub fn lt<T: AsRef<[u8]>>(self, bound: T) -> Self {
        StreamBuilder(self.0.lt(bound))
    }
}

impl<'s, 'a, A: Automaton> IntoStreamer<'a> for StreamBuilder<'s, A> {
    type Item = (&'a [u8], u64);
    type Into = Stream<'s, A>;

    fn into_stream(self) -> Self::Into {
        Stream(self.0.into_stream())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fuzzy::util::multi_modified_damlev;

    #[test]
    fn lookup_test_cases_d_1() {
        extern crate tempfile;
        //building the structure with https://raw.githubusercontent.com/BurntSushi/fst/master/data/words-10000
        let data = reqwest::get("https://raw.githubusercontent.com/BurntSushi/fst/master/data/words-10000")
        .expect("tried to download data")
        .text().expect("tried to decode the data");
        let mut words = data.trim().split("\n").collect::<Vec<&str>>();
        words.sort();

        let expect = |word: &'static str, query: &'static str| {
            FuzzyMapLookupResult { word: word.to_owned(), id: words.binary_search(&word).unwrap() as u32, edit_distance: multi_modified_damlev(&word, &[&query])[0] as u8 }
        };

        let no_return = Vec::<FuzzyMapLookupResult>::new();

        let dir = tempfile::tempdir().unwrap();
        let file_start = dir.path().join("fuzzy");
        FuzzyMapBuilder::build_from_iter(&file_start, words.iter().cloned(), 1).unwrap();

        let map = unsafe { FuzzyMap::from_path(&file_start).unwrap() };
        let query = "alazan";
        let matches = map.lookup(&query, 1, |id| &words[id as usize]);
        assert_eq!(matches.unwrap(), [expect("albazan", query)]);

        //exact lookup, the original word in the data is - "agߪkaधaݤcݤkaqag"
        let query = "agߪkaधaݤcݤkaqag";
        let matches = map.lookup(&query, 1, |id| &words[id as usize]);
        assert_eq!(matches.unwrap(), [expect("agߪkaधaݤcݤkaqag", query)]);

        //not exact lookup, the original word is - "blockquoteanciently", d=1
        let query = "blockquteanciently";
        let matches = map.lookup(&query, 1, |id| &words[id as usize]);
        assert_eq!(matches.unwrap(), [expect("blockquoteanciently", query)]);

        //not exact lookup, d=1, more more than one suggestion because of two similiar words in the data
        //albana and albazan
        let query = "albaza";
        let matches = map.lookup(&query, 1, |id| &words[id as usize]);
        assert_eq!(matches.unwrap(), [expect("albana", query), expect("albazan", query)]);

        //include a test that explores multiple results that share an fst entry
        let query = "fern";
        let matches = map.lookup(&query, 1, |id| &words[id as usize]);
        assert_eq!(matches.unwrap(), [expect("farn", query), expect("fernd", query), expect("ferni", query)]);

        //garbage input
        let query = "🤔";
        let matches = map.lookup(&query, 1, |id| &words[id as usize]);
        assert_eq!(matches.unwrap(), no_return);

        let query = "";
        let matches = map.lookup(&query, 1, |id| &words[id as usize]);
        assert_eq!(matches.unwrap(), no_return);
    }

    #[test]
    fn lookup_test_cases_d_2() {
        extern crate tempfile;
        let words = vec!["100", "main", "street"];

        let expect = |word: &'static str, query: &'static str| {
            FuzzyMapLookupResult { word: word.to_owned(), id: words.binary_search(&word).unwrap() as u32, edit_distance: multi_modified_damlev(&word, &[&query])[0] as u8 }
        };

        let dir = tempfile::tempdir().unwrap();
        let file_start = dir.path().join("fuzzy");
        FuzzyMapBuilder::build_from_iter(&file_start, words.iter().cloned(), 2).unwrap();

        let map = unsafe { FuzzyMap::from_path(&file_start).unwrap() };
        let query = "sret";
        let matches = map.lookup(&query, 2, |id| &words[id as usize]);
        assert_eq!(matches.unwrap(), [expect("street", query)])
    }
}
