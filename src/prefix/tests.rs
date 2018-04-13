#[cfg(test)] extern crate reqwest;
use super::PrefixSet;
use fst::raw;

#[test]
fn test() {
    let data = reqwest::get("https://raw.githubusercontent.com/BurntSushi/fst/master/data/words-10000")
        .expect("tried to download data")
        .text().expect("tried to decode the data");
    let words = data.trim().split("\n").collect::<Vec<&str>>();

    let pf = PrefixSet::from_iter(words.iter()).expect("tried to create prefix set");

    assert_eq!(pf.len(), words.len(), "PrefixSet contains the right number of words");

    let words_with_ids = words.iter().enumerate()
        .map(|(i, w)| (w.to_string(), i as u64)).collect::<Vec<(String, u64)>>();

    assert_eq!(
        pf.stream().into_str_vec().expect("tried to dump to vector"),
        words_with_ids,
        "PrefixSet's IDs match the lexicographical IDs of the original data"
    );

    assert!(
        words.iter().all(|w| pf.contains(w)),
        "PrefixSet contains all words"
    );

    assert!(
        words.iter().all(|w| pf.contains_prefix(w)),
        "PrefixSet contains all words as prefixes"
    );

    assert!(
        words.iter().all(|w| {
            let char_count = w.chars().count();
            let prefix: String = w.chars().take(char_count - 1).collect();
            pf.contains_prefix(prefix)
        }),
        "PrefixSet contains prefixes of all words as prefixes"
    );

    assert!(
        words_with_ids.iter().all(|ref t| {
            match pf.get_by_id(raw::Output::new(t.1)) {
                Some(v) => match String::from_utf8(v) {
                    Ok(s) => s == t.0,
                    _ => false
                },
                None => false
            }
        }),
        "PrefixSet inverse lookups return the expected result"
    );

    assert!(
        pf.get_by_id(raw::Output::new(words.len() as u64)).is_none(),
        "PrefixSet inverse lookup returns none on out of bounds lookup"
    );

    let be_subset: Vec<(String, u64)> = words_with_ids.iter().filter(|ref t| t.0.starts_with("be")).cloned().collect();
    let be_range = pf.get_prefix_range("be").unwrap();
    assert_eq!(
        (be_range.0.value(), be_range.1.value()),
        (be_subset[0].1, be_subset.last().unwrap().1 + 1),
        "Prefix range for string 'be' behaves as expected"
    );

    let plus_qq: Vec<String> = words.iter().map(|w| w.to_string() + "qq").collect();

    assert!(
        plus_qq.iter().all(|w| !pf.contains(w)),
        "PrefixSet contains no words appended with 'qq' at the end"
    );

    assert!(
        plus_qq.iter().all(|w| !pf.contains_prefix(w)),
        "PrefixSet contains no words appended with 'qq' at the end as prefixes"
    );

    assert!(
        plus_qq.iter().all(|w| pf.get(w).is_none()),
        "PrefixSet can't get any words appended with 'qq' at the end"
    );

    assert!(
        plus_qq.iter().all(|w| pf.get_prefix_range(w).is_none()),
        "PrefixSet can't get prefix range of any words appended with 'qq' at the end"
    );
}