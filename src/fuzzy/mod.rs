//use fst::{IntoStreamer, Streamer, Set, Map, MapBuilder, Automaton};
use std::*;
#[cfg(test)] extern crate reqwest;


#[derive(Debug)]
struct VectorCollection(Vec<String>);

impl VectorCollection {
    fn new() -> VectorCollection {
        VectorCollection(Vec::new())
    }
}

// and we'll implement IntoIterator
impl IntoIterator for VectorCollection {
    type Item = u8;
    type IntoIter = ::std::vec::IntoIter<u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_iter()
    }
}

struct Symspell {
    word_list: Vec<String>,
    id_list: Vec<Vec<usize>>
}

impl Symspell {
    // will only build the structure
    fn build(&self) {
        println!("{:?}, {:?}", self.word_list, self.id_list);
    }
    //creates delete variants for every word in the list
    fn create_variants<T>(words: T) -> Vec<(String, usize)> where T: IntoIterator {

        let mut word_variants = Vec::<(String, usize)>::new();
        //treating &words as a slice, since, slices are read-only objects
        for (i, &word) in words.into_iter().enumerate() {
        //let x: () = (*word).to_owned();
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
}

#[test]
fn use_symspell() {
    let data = reqwest::get("https://raw.githubusercontent.com/BurntSushi/fst/master/data/words-10000")
       .expect("tried to download data")
       .text().expect("tried to decode the data");
    let mut words = data.trim().split("\n").collect::<Vec<&str>>();
    words.sort();
    //create variants
    Symspell::create_variants(&words);
}

fn main() {
}
