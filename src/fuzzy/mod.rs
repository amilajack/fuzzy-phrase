use std::io::{BufWriter};
use std::fs::File;
use std::error::Error;
use itertools::Itertools;
use strsim::damerau_levenshtein;

mod map;
pub use self::map::FuzzyMapBuilder;
pub use self::map::FuzzyMap;

static BIG_NUMBER: usize = 1 << 30;

#[cfg(test)] extern crate reqwest;
#[derive(Debug, PartialEq)]

pub struct Symspell {
    id_list: Vec<Vec<usize>>
}

impl Symspell {
    pub fn new(id_list: Vec<Vec<usize>>) -> Symspell {
        Symspell { id_list: id_list }
    }

    //builds the graph and writes to disk, additionally writes the ids to id_list which is a part of struct
    fn build<'a, T>(words: T, edit_distance: u64) -> Result<Vec<Vec<usize>>, Box<Error>> where T: IntoIterator<Item=&'a &'a str> {
        let word_variants = Symspell::create_variants(words, edit_distance);
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
        let multi_idx = Symspell::new(multids.to_vec());
        build.finish()?;
        Ok(multi_idx.id_list)
    }
    //creates delete variants for every word in the list
    //using usize for - https://stackoverflow.com/questions/29592256/whats-the-difference-between-usize-and-u32?utm_medium=organic&utm_source=google_rich_qa&utm_campaign=google_rich_qa
    fn create_variants<'a, T>(words: T, edit_distance: u64) -> Vec<(String, usize)> where T: IntoIterator<Item=&'a &'a str> {
        let mut word_variants = Vec::<(String, usize)>::new();
        //treating &words as a slice, since, slices are read-only objects
        for (i, &word) in words.into_iter().enumerate() {

            word_variants.push((word.to_owned(), i));
            for k in 1..edit_distance + 1 {
                for (j, _) in word.char_indices() {
                    let mut s = String::with_capacity(word.len() - 1);
                    let parts = word.split_at(j);
                    s.push_str(parts.0);
                    s.extend(parts.1.chars().skip(k as usize));
                    word_variants.push((s, i));
                }
            }
        }
        word_variants.sort();
        word_variants.dedup();
        word_variants
    }

    //Defining lifetimes here because we are expecting the string to last the lifetime of the closure F
    fn lookup<'a, F>(query: &str, edit_distance: u64, ids: &Vec<Vec<usize>>, lookup_fn: F) -> Result<Vec<String>, Box<Error>> where F: Fn(usize) -> &'a str {

        let levenshtein_limit : usize;
        let mut query_variants = Vec::new();
        let mut matches = Vec::<usize>::new();

        //read all the bytes in the fst file
        let map = unsafe { FuzzyMap::from_path("x_sym.fst")? };

        //create variants of the query itself
        query_variants.push(query.to_owned());
        for k in 1..edit_distance + 1 {
            for (j, _) in query.char_indices() {
                let mut variant = String::with_capacity(query.len() - 1);
                let parts = query.split_at(j);
                variant.push_str(parts.0);
                variant.extend(parts.1.chars().skip(k as usize));
                query_variants.push(variant);
            }
        }
        query_variants.sort();
        query_variants.dedup();

        for i in query_variants {
            match map.get(&i) {
                Some (idx) => {
                    let uidx = idx as usize;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_test_cases_d_1() {
        //building the structure with https://raw.githubusercontent.com/BurntSushi/fst/master/data/words-10000
        let data = reqwest::get("https://raw.githubusercontent.com/BurntSushi/fst/master/data/words-10000")
        .expect("tried to download data")
        .text().expect("tried to decode the data");
        let mut words = data.trim().split("\n").collect::<Vec<&str>>();
        words.sort();
        let no_return = Vec::<String>::new();

        //building the structure
        let ids = Symspell::build(&words, 1);
        let unwrapped_ids = &ids.unwrap();
        //exact lookup, the original word in the data is - "albazan"
        let query1 = "albazan";
        let matches = Symspell::lookup(&query1, 1, unwrapped_ids, |id| &words[id]);
        assert_eq!(matches.unwrap(), ["albazan"]);

        //exact lookup, the original word in the data is - "agߪkaधaݤcݤkaqag"
        let query2 = "agߪkaधaݤcݤkaqag";
        let matches = Symspell::lookup(&query2, 1, unwrapped_ids, |id| &words[id]);
        assert_eq!(matches.unwrap(), ["agߪkaधaݤcݤkaqag"]);

        //not exact lookup, the original word is - "blockquoteanciently", d=1
        let query3 = "blockquteanciently";
        let matches = Symspell::lookup(&query3, 1, unwrapped_ids, |id| &words[id]);
        assert_eq!(matches.unwrap(), ["blockquoteanciently"]);

        //not exact lookup, d=1, more more than one suggestion because of two similiar words in the data
        //albana and albazan
        let query4 = "albaza";
        let matches = Symspell::lookup(&query4, 1, unwrapped_ids, |id| &words[id]);
        assert_eq!(matches.unwrap(), ["albana", "albazan"]);

        //garbage input
        let query4 = "🤔";
        let matches = Symspell::lookup(&query4, 1, unwrapped_ids, |id| &words[id]);
        assert_eq!(matches.unwrap(), no_return);
    }
}
