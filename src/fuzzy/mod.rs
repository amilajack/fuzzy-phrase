use std::io::{BufReader, BufWriter};
use std::fs::File;
use std::error::Error;
use itertools::Itertools;
use strsim::damerau_levenshtein;
use serde::{Deserialize, Serialize};
use rmps::{Deserializer, Serializer};

mod map;
pub use self::map::FuzzyMapBuilder;
pub use self::map::FuzzyMap;

static BIG_NUMBER: usize = 1 << 30;

#[cfg(test)] extern crate reqwest;
#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[derive(Clone)]

struct Symspell {
    id_list: Vec<Vec<usize>>
}

impl Symspell {
    //builds the structure
    fn build<'a, T>(words: T) -> Result<(), Box<Error>> where T: IntoIterator<Item=&'a &'a str> {
        let word_variants = Symspell::create_variants(words);
        let wtr = BufWriter::new(File::create("x_sym.fst")?);
        let mut build = FuzzyMapBuilder::new(wtr)?;
        let mut multids = Vec::<Vec<usize>>::new();
        for (key, group) in &(&word_variants).iter().dedup().group_by(|t| &t.0) {
            let opts = group.collect::<Vec<_>>();
            let id = if opts.len() == 1 {
                opts[0].1
            } else {
                multids.push((&opts).iter().map(|t| t.1).collect::<Vec<_>>());
                multids.len() - 1 + BIG_NUMBER
            };
            build.insert(key, id as u64)?;
        }
        let multi_idx = Symspell { id_list: multids.to_vec() };
        let mf_wtr = BufWriter::new(File::create("id.msg")?);
        multi_idx.serialize(&mut Serializer::new(mf_wtr))?;
        build.finish()?;
        Ok(())
    }
    //creates delete variants for every word in the list
    //using usize for - https://stackoverflow.com/questions/29592256/whats-the-difference-between-usize-and-u32?utm_medium=organic&utm_source=google_rich_qa&utm_campaign=google_rich_qa
    fn create_variants<'a, T>(words: T) -> Vec<(String, usize)> where T: IntoIterator<Item=&'a &'a str> {
        let mut word_variants = Vec::<(String, usize)>::new();
        //treating &words as a slice, since, slices are read-only objects
        for (i, &word) in words.into_iter().enumerate() {

            word_variants.push((word.to_owned(), i));
            for (j, _) in word.char_indices() {
                let mut s = String::with_capacity(word.len() - 1);
                let parts = word.split_at(j);
                s.push_str(parts.0);
                s.extend(parts.1.chars().skip(1));
                word_variants.push((s, i));
            }
        }
        word_variants.sort();
        word_variants
    }

    //Defining lifetimes here because we are expecting the string to last the lifetime of the closure F
    fn lookup<'a, F>(query: &str, lookup_fn: F) -> Result<Vec<String>, Box<Error>> where F: Fn(usize) -> &'a str {

        let mut query_variants = Vec::new();
        let mut matches = Vec::<usize>::new();

        //read all the bytes in the fst file
        let map = unsafe { FuzzyMap::from_path("x_sym.fst")? };

        //create variants of the query itself
        query_variants.push(query.to_owned());
        for (j, _) in query.char_indices() {
            let mut variant = String::with_capacity(query.len() - 1);
            let parts = query.split_at(j);
            variant.push_str(parts.0);
            variant.extend(parts.1.chars().skip(1));
            query_variants.push(variant);
        }

        let mf : Symspell;
        let mf_file = File::open("id.msg")?;
        let mf_reader = BufReader::new(mf_file);
        mf = Deserialize::deserialize(&mut Deserializer::new(mf_reader))?;

        for i in query_variants {
            match map.get(&i) {
                Some (idx) => {
                    let uidx = idx as usize;
                    if uidx < BIG_NUMBER {
                        matches.push(uidx);
                    } else {
                        for x in &(mf.id_list)[uidx - BIG_NUMBER] {
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
        Ok(matches
            .into_iter().dedup()
            .map(lookup_fn)
            .filter(|word| damerau_levenshtein(query, word) < 2)
            .map(|word| word.to_owned())
            .collect::<Vec<String>>()
        )
    }
}

#[test]
fn exact_match() {
    //the original word in the data is - "ALBAZAN"
    let query = "lbazan";
    let data = reqwest::get("https://raw.githubusercontent.com/BurntSushi/fst/master/data/words-10000")
    .expect("tried to download data")
    .text().expect("tried to decode the data");
    let mut words = data.trim().split("\n").collect::<Vec<&str>>();
    words.sort();
    let _built = Symspell::build(&words);
    let matches = Symspell::lookup(&query, |id| &words[id]);
    assert_eq!(matches.unwrap(), ["albazan"]);
}
