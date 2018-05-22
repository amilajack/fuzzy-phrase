use std::collections::HashSet;
mod map;
pub use self::map::FuzzyMap;
pub use self::map::FuzzyMapBuilder;

#[cfg(test)] extern crate reqwest;

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
        let wtr = FuzzyMapBuilder::new("/tmp/");
        wtr.build(&words, 1);

        let unwrapped_ids = &ids.unwrap();
        //exact lookup, the original word in the data is - "albazan"
        let query1 = "alazan";
        let matches = FuzzyMap::lookup(&query1, 1, unwrapped_ids, |id| &words[id]);
        assert_eq!(matches.unwrap(), ["albazan"]);

        //exact lookup, the original word in the data is - "agߪkaधaݤcݤkaqag"
        // let query2 = "agߪkaधaݤcݤkaqag";
        // let matches = Symspell::FuzzyMap(&query2, 1, unwrapped_ids, |id| &words[id]);
        // assert_eq!(matches.unwrap(), ["agߪkaधaݤcݤkaqag"]);
        //
        // //not exact lookup, the original word is - "blockquoteanciently", d=1
        // let query3 = "blockquteanciently";
        // let matches = Symspell::FuzzyMap(&query3, 1, unwrapped_ids, |id| &words[id]);
        // assert_eq!(matches.unwrap(), ["blockquoteanciently"]);
        //
        // //not exact lookup, d=1, more more than one suggestion because of two similiar words in the data
        // //albana and albazan
        // let query4 = "albaza";
        // let matches = Symspell::FuzzyMap(&query4, 1, unwrapped_ids, |id| &words[id]);
        // assert_eq!(matches.unwrap(), ["albana", "albazan"]);
        //
        // //garbage input
        // let query4 = "🤔";
        // let matches = Symspell::FuzzyMap(&query4, 1, unwrapped_ids, |id| &words[id]);
        // assert_eq!(matches.unwrap(), no_return);
        //
        // let query5 = "";
        // let matches = Symspell::FuzzyMap(&query5, 1, unwrapped_ids, |id| &words[id]);
        // assert_eq!(matches.unwrap(), no_return);
    }
}
