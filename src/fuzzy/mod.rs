use std::io::{BufWriter};
use std::fs::File;
use std::error::Error;
use itertools::Itertools;
use std::collections::HashSet;
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
        let word_variants = create_variants(words, edit_distance);
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

    //Defining lifetimes here because we are expecting the string to last the lifetime of the closure F
    fn lookup<'a, F>(query: &str, edit_distance: u64, ids: &Vec<Vec<usize>>, lookup_fn: F) -> Result<Vec<String>, Box<Error>> where F: Fn(usize) -> &'a str {
        let mut e_flag: u64 = 1;
        if edit_distance == 1 { e_flag = 2; }
        let levenshtein_limit : usize;
        let mut query_variants = Vec::new();
        let mut matches = Vec::<usize>::new();

        //read all the bytes in the fst file
        let map = unsafe { FuzzyMap::from_path("x_sym.fst")? };

        //create variants of the query itself
        query_variants.push(query.to_owned());
        let mut variants: HashSet<String> = HashSet::new();
        let all_query_variants = edits(&query, e_flag, 2, &mut variants);
        for j in all_query_variants.iter() {
            query_variants.push(j.to_owned());
        }
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

//creates delete variants for every word in the list
//using usize for - https://stackoverflow.com/questions/29592256/whats-the-difference-between-usize-and-u32?utm_medium=organic&utm_source=google_rich_qa&utm_campaign=google_rich_qa
fn create_variants<'a, T>(words: T, edit_distance: u64) -> Vec<(String, usize)> where T: IntoIterator<Item=&'a &'a str> {
    let mut word_variants = Vec::<(String, usize)>::new();
    let mut e_flag: u64 = 1;
    if edit_distance == 1 { e_flag = 2; }

    //treating &words as a slice, since, slices are read-only objects
    for (i, &word) in words.into_iter().enumerate() {
        word_variants.push((word.to_owned(), i));
        let mut variants: HashSet<String> = HashSet::new();
        let all_variants = edits(&word, e_flag, 2, &mut variants);
        for j in all_variants.iter() {
            word_variants.push((j.to_owned(), i));
        }
    }
    word_variants.sort();
    word_variants
}

fn edits<'a>(word: &str, edit_distance: u64, max_distance: u64, delete_variants: &'a mut HashSet<String>) -> &'a mut HashSet<String> {
    let mut iter = word.char_indices().peekable();

    while let Some((pos, _char)) = iter.next() {
        let mut deleted_item = String::with_capacity(word.len());
        deleted_item.push_str(&word[..pos]);

        if let Some((next_pos, _)) = iter.peek() {
            deleted_item.push_str(&word[*next_pos..]);
        }

        if edit_distance < max_distance {
            edits(&deleted_item, edit_distance + 1, max_distance, delete_variants);
        }

        delete_variants.insert(deleted_item);
    }
    delete_variants
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
        let query1 = "alazan";
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

        let query5 = "";
        let matches = Symspell::lookup(&query5, 1, unwrapped_ids, |id| &words[id]);
        assert_eq!(matches.unwrap(), no_return);
    }
}
