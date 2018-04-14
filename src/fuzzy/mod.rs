extern crate fst;
use fst::{IntoStreamer, Streamer, Set, Map, MapBuilder, Automaton};

struct Symspell {
    map: &Map,
    words: &Vec<String>,
    id: &Vec<Vec<usize>>
}
