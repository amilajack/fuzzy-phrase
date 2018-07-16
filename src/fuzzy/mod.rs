use std::collections::HashSet;
pub mod map;
mod util;
pub use self::map::FuzzyMap;
pub use self::map::FuzzyMapBuilder;

#[inline(always)]
fn get_variants<'a>(word: &str, edit_distance: u8) -> HashSet<String> {
    let mut variants: HashSet<String> = HashSet::new();
    get_variants_recursive(word, 1, edit_distance, &mut variants);
    variants
}

fn get_variants_recursive<'a>(word: &str, edit_distance: u8, max_distance: u8, delete_variants: &'a mut HashSet<String>) -> () {
    let mut iter = word.char_indices().peekable();

    while let Some((pos, _char)) = iter.next() {
        let mut deleted_item = String::with_capacity(word.len());
        deleted_item.push_str(&word[..pos]);

        if let Some((next_pos, _)) = iter.peek() {
            deleted_item.push_str(&word[*next_pos..]);
        }

        if edit_distance < max_distance {
            get_variants_recursive(&deleted_item, edit_distance + 1, max_distance, delete_variants);
        }
        delete_variants.insert(deleted_item);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn get_variants_test_edit_distance_1() {
        let query = "hello";
        let query_variants = get_variants(query, 1);
        let mut result = HashSet::new();
        result.insert("helo".to_owned());
        result.insert("hell".to_owned());
        result.insert("ello".to_owned());
        result.insert("hllo".to_owned());
        assert_eq!(query_variants, result);
    }

    #[test]
    fn get_variants_test_edit_distance_2() {
        let query = "hello";
        let query_variants = get_variants(query, 2);
        let mut result = HashSet::new();
        result.insert("helo".to_owned());
        result.insert("hell".to_owned());
        result.insert("ello".to_owned());
        result.insert("hllo".to_owned());
        result.insert("elo".to_owned());
        result.insert("ell".to_owned());
        result.insert("hel".to_owned());
        result.insert("hll".to_owned());
        result.insert("hlo".to_owned());
        result.insert("heo".to_owned());
        result.insert("llo".to_owned());
        assert_eq!(query_variants, result);
    }
}
