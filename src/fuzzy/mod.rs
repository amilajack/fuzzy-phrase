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
