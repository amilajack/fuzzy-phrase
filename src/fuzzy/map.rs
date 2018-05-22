use fst::{IntoStreamer, Streamer, Automaton};
use std;
use std::collections::HashSet;
use std::error::Error;
use itertools::Itertools;
use fst::raw;
use fst::Error as FstError;
use fst::automaton::{AlwaysMatch};
#[cfg(feature = "mmap")]
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{BufWriter};
use serde::{Serialize};
use rmps::{Serializer};
use strsim::damerau_levenshtein;

static BIG_NUMBER: usize = 1 << 30;

#[derive(Debug, PartialEq, Deserialize, Serialize)]
struct MultiIdxList(Vec<Vec<usize>>);

pub struct FuzzyMap(raw::Fst, MultiIdxList);

impl FuzzyMap {
    pub fn new(fst: raw::Fst, id_list: Vec<Vec<usize>>) -> Result<Self, FstError> {
        Ok(FuzzyMap(fst, MultiIdxList(id_list)))
    }

    #[cfg(feature = "mmap")]
    pub unsafe fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, FstError> {
        //this to 1 path
        let fst = raw::Fst::from_path(path).unwrap();
        //Deserialize it and store in id_list
        let id_list = Vec::<Vec<usize>>::new();
        FuzzyMap::new(fst, id_list)
    }

    pub fn contains<K: AsRef<[u8]>>(&self, key: K) -> bool {
        self.0.contains_key(key)
    }

    pub fn stream(&self) -> Stream {
        Stream(self.0.stream())
    }

    pub fn range(&self) -> StreamBuilder {
        StreamBuilder(self.0.range())
    }

    pub fn search<A: Automaton>(&self, aut: A) -> StreamBuilder<A> {
        StreamBuilder(self.0.search(aut))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_fst(&self) -> &raw::Fst {
        &self.0
    }

    // this one is from Map
    pub fn get<K: AsRef<[u8]>>(&self, key: K) -> Option<u64> {
        self.0.get(key).map(|output| output.value())
    }
    //get rid of ids
    pub fn lookup<'a, F>(&self, query: &str, edit_distance: u64, ids: &Vec<Vec<usize>>, lookup_fn: F) -> Result<Vec<String>, Box<Error>> where F: Fn(usize) -> &'a str {
        let mut e_flag: u64 = 1;
        if edit_distance == 1 { e_flag = 2; }
        let levenshtein_limit : usize;
        let mut query_variants = Vec::new();
        let mut matches = Vec::<usize>::new();

        //create variants of the query itself
        query_variants.push(query.to_owned());
        let mut variants: HashSet<String> = HashSet::new();
        let all_query_variants = super::edits(&query, e_flag, 2, &mut variants);
        for j in all_query_variants.iter() {
            query_variants.push(j.to_owned());
        }
        query_variants.dedup();

        for i in query_variants {
            match self.0.get(&i) {
                Some (idx) => {
                    let uidx = idx.value() as usize;
                    if uidx < BIG_NUMBER {
                        matches.push(uidx);
                    } else {
                       for x in &(ids)[uidx - BIG_NUMBER] {
                            matches.push(*x);
                        }
                    }
                }
                None => {}
            }
        }
        //return all ids that match
        matches.sort();

        //checks all words whose damerau levenshtein edit distance is lesser than 2
        if edit_distance == 1 {
            levenshtein_limit = 2;
        } else { levenshtein_limit = 3; }


        Ok(matches
            .into_iter().dedup()
            .map(lookup_fn)
            .filter(|word| damerau_levenshtein(query, word) < levenshtein_limit as usize)
            .map(|word| word.to_owned())
            .collect::<Vec<String>>()
        )
    }
}

// #[derive(Debug, PartialEq, Deserialize, Serialize)]
// struct MultiIdBuilder(Vec<Vec<usize>>);

pub struct FuzzyMapBuilder {
    builder: raw::Builder<BufWriter<File>>,
    id_builder: Vec<Vec<usize>>,
    prefixFilePath: PathBuf
}

impl FuzzyMapBuilder {
    //takes one path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<FuzzyMapBuilder, FstError> where P: std::convert::AsRef<std::ffi::OsStr> {
        let filePath = path.as_ref().to_owned();
        let mut fst_wtr = BufWriter::new(File::create(Path::new(&path).join(".fst"))?);
        let mut id_wtr = BufWriter::new(File::create(Path::new(&path).join(".msg"))?);
        Ok(FuzzyMapBuilder { builder: raw::Builder::new_type(fst_wtr, 0)?, id_builder: Vec::<Vec<usize>>::new(), prefixFilePath: filePath })
    }

    pub fn build<'a, T>(&mut self, words: T, edit_distance: u64) -> Result<(), Box<Error>> where T: IntoIterator<Item=&'a &'a str> {
        let word_variants = super::create_variants(words, edit_distance);
        //precalculate the file path;
        for (key, group) in &(&word_variants).iter().dedup().group_by(|t| &t.0) {
            let opts = group.collect::<Vec<_>>();
            self.insert(key, opts)?
        }
        self.finish()?;
        Ok(())
    }

    pub fn insert<K: AsRef<[u8]>>(&mut self, key: K, ids: Vec<usize>) -> Result<(), FstError> {
        let id = if ids.len() == 1 {
            ids[0]
        } else {
            self.id_builder.push((&ids).iter().map(|t| 1).collect::<Vec<_>>());
            self.id_builder.len() - 1 + BIG_NUMBER
        };
        self.builder.insert(key, id as u64)?;
        Ok(())
    }

    pub fn extend_iter<T, I>(&mut self, iter: I) -> Result<(), FstError> where T: AsRef<[u8]>, I: IntoIterator<Item=(T, u64)> {
        for (k, v) in iter {
            self.builder.insert(k, v)?;
        }
        Ok(())
    }

    pub fn finish(self) -> Result<(), FstError> {
        //based on the path internally - use the constructor path
        let filePath: FuzzyMapBuilder.path;
        let mut mf_wtr = BufWriter::new(File::create("midx.msg"));
        self.id_builder.serialize(&mut Serializer::new(mf_wtr)?);
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
