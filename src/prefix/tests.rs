extern crate lazy_static;

use std::collections::BTreeSet;
use super::PrefixSet;
use fst::raw;

lazy_static! {
    static ref DATA: [&'static str; 4] = [
        include_str!("../../benches/data/phrase_test_shared_prefix.txt"),
        include_str!("../../benches/data/phrase_test_typos.txt"),
        include_str!("../../benches/data/phrase_test_cities_ar.txt"),
        include_str!("../../benches/data/phrase_test_cities_ru.txt"),
    ];
    static ref WORD_SET: BTreeSet<&'static str> = {
        let mut bts: BTreeSet<&'static str> = BTreeSet::new();
        for data in DATA.iter() {
            let phrases = data.trim().split("\n").collect::<Vec<&str>>();
            for phrase in phrases {
                let words = phrase.trim().split(" ");
                for word in words {
                    bts.insert(word);
                }
            }
        }
        bts
    };
    static ref WORDS: Vec<&'static str> = {
        WORD_SET.iter().cloned().collect()
    };
    static ref WORDS_WITH_CONTINUATIONS: BTreeSet<&'static str> = {
        // take each word in the set, seek to the range that starts with that word,
        // and filter to only things that start with that word but aren't that word
        WORD_SET.iter().filter(|&w| {
            WORD_SET
                .range((*w)..)
                .take_while(|p| p.starts_with(w))
                .filter(|&p| p != w)
                .count() > 0
        }).cloned().collect()
    };
    static ref WORDS_WITH_IDS: Vec<(String, u64)> = {
        WORDS.iter().enumerate()
            .map(|(i, w)| (w.to_string(), i as u64)).collect::<Vec<(String, u64)>>()
    };
    static ref SET: PrefixSet = {
        PrefixSet::from_iter(WORDS.iter()).expect("tried to create prefix set")
    };
}

#[test]
fn simple_build() {
    let mut words = vec!["one", "two", "three"];
    words.sort();

    let pf = PrefixSet::from_iter(words.iter()).expect("tried to create prefix set");
    assert_eq!(format!("{:?}", pf), "PrefixSet([(one, 0), (three, 1), (two, 2)])");
}

#[test]
fn complex_build() {
    lazy_static::initialize(&SET);
}

#[test]
fn confirm_contents() {
    assert_eq!(SET.len(), WORDS.len(), "PrefixSet contains the right number of WORDS");

    assert_eq!(
        SET.stream().into_str_vec().expect("tried to dump to vector"),
        *WORDS_WITH_IDS,
        "PrefixSet's IDs match the lexicographical IDs of the original data"
    );
}

#[test]
fn contains() {
    assert!(
        WORDS.iter().all(|w| {
            let lookup = SET.lookup(w);
            lookup.found() && lookup.found_final()
        }),
        "PrefixSet contains all WORDS as prefixes and full words"
    );

    assert!(
        WORDS.iter().all(|w| {
            let lookup = SET.lookup(w);
            if WORDS_WITH_CONTINUATIONS.contains(w) {
                lookup.has_continuations()
            } else {
                !lookup.has_continuations()
            }
        }),
        "PrefixSet's words that should have continuations do, and the ones that shouldn't, don't"
    );
}

#[test]
fn contains_prefix() {
    assert!(
        WORDS.iter().all(|w| {
            let char_count = w.chars().count();
            let prefix: String = w.chars().take(char_count - 1).collect();
            let lookup = SET.lookup(prefix);
            lookup.found() && lookup.has_continuations()
        }),
        "PrefixSet contains prefixes of all WORDS as prefixes, and they all have continuations"
    );

    assert!(
        WORDS_WITH_IDS.iter().all(|ref t| {
            match SET.get_by_id(raw::Output::new(t.1)) {
                Some(v) => match String::from_utf8(v) {
                    Ok(s) => s == t.0,
                    _ => false
                },
                None => false
            }
        }),
        "PrefixSet inverse lookups return the expected result"
    );

    let co_subset: Vec<(String, u64)> = WORDS_WITH_IDS.iter().filter(|ref t| t.0.starts_with("Co")).cloned().collect();
    let co_range = SET.lookup("Co").range().unwrap();
    assert_eq!(
        (co_range.0.value(), co_range.1.value()),
        (co_subset[0].1, co_subset.last().unwrap().1),
        "Prefix range for string 'be' behaves as expected"
    );
}

#[test]
fn augmented_contains() {
    let plus_qq: Vec<String> = WORDS.iter().map(|w| w.to_string() + "qq").collect();

    assert!(
        plus_qq.iter().all(|w| !SET.lookup(w).found_final()),
        "PrefixSet contains no WORDS appended with 'qq' at the end"
    );

    assert!(
        plus_qq.iter().all(|w| !SET.lookup(w).found()),
        "PrefixSet contains no WORDS appended with 'qq' at the end as prefixes"
    );

    assert!(
        plus_qq.iter().all(|w| !SET.lookup(w).found()),
        "PrefixSet can't get any WORDS appended with 'qq' at the end"
    );

    assert!(
        plus_qq.iter().all(|w| SET.lookup(w).range().is_none()),
        "PrefixSet can't get prefix range of any WORDS appended with 'qq' at the end"
    );
}

#[test]
fn get_by_id() {
    assert!(
        SET.get_by_id(raw::Output::new(WORDS.len() as u64)).is_none(),
        "PrefixSet inverse lookup returns none on out of bounds lookup"
    );
}