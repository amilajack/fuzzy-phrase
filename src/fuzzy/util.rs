use std::cmp::min;

/// This functions implements modified Damerau-Levenshtein distance (also called
/// Damerau-Levenshtein optimal string alignment). It calculates the edit distance between strings,
/// where edits can consist of insertion, deletion, substitution, or transposition, but unlike
/// traditional Damerau-Levenshtein, it does not consider edit sequences where a transposition is
/// followed by further edits that affect the transposed letters (for example, two adjacent
/// subsequent transpositions, or a transposition followed by an insert in between the transposed
/// letters), and as such, it no longer satisfies the triangle inequality. For example, d(CA, AC) ==
/// 1, d(AC, ABC) == 1, but d(CA, ABC) == 3. In return, however, it's significantly faster than
/// regular D-L.
///
/// This implementation is takes a mix of inspirations, including from the
/// pseudocode here:
/// https://en.wikipedia.org/wiki/Damerau%E2%80%93Levenshtein_distance#Optimal_string_alignment_distance
/// and also a strategy for only keeping the last three rows rather than the whole matrix, which
/// appears in several implementations including the osa_distance implementation in simstring, plus
/// some alterations to avoid work repetition when comparing the same target string to multiple
/// different candidate matches, as we do in the context of Symspell lookups. It avoids repeating
/// Unicode parses if possible, and also reuses vectors to store distance information across
/// multiple candidate words. The traditional implementation requires maintaining a len(a)*len(b)
/// matrix across all comparisons, but in fact only the most recently touched three rows of that
/// matrix are necessary, and they can be shifted/reused to avoid requiring fresh allocations.
/// Further, we can choose which of the two words we're comparing dictates our row size, and if we
/// choose the target word, the vectors can stay the same size across all candidate words.

#[allow(dead_code)]
#[inline(always)]
pub fn multi_modified_damlev<T: AsRef<str>>(target: T, sources: &[T]) -> Vec<u32> {
    multi_modified_damlev_hint(target, sources, u32::max_value())
}

/// This is a variant of the main D-L function with slightly relaxed guarantees: you supply a hint
/// for the maximum distance you care about, and for any pairs that are farther apart than that,
/// you're guaranteed a result that's greater than your hinted max, but it might not be the actual
/// distance.

pub fn multi_modified_damlev_hint<T: AsRef<str>>(target: T, sources: &[T], max_hint: u32) -> Vec<u32> {
    let t_chars: Vec<char> = target.as_ref().chars().collect();
    let t_len = t_chars.len();

    if t_len == 0 {
        return sources.iter().map(|s| s.as_ref().chars().count() as u32).collect();
    }

    let width = t_len + 1;
    let mut cur_row: Vec<u32> = vec![0; width];
    let mut prev_row: Vec<u32> = vec![0; width];
    let mut prev2_row: Vec<u32> = vec![0; width];

    let mut out: Vec<u32> = Vec::with_capacity(sources.as_ref().len());
    let mut s_chars: Vec<char> = Vec::with_capacity(t_len + 1);
    for s in sources {
        s_chars.clear();
        s_chars.extend(s.as_ref().chars());
        let s_len = s_chars.len();

        if t_chars == s_chars {
            out.push(0);
            continue;
        } else if s_len == 0 {
            out.push(t_len as u32);
            continue;
        }

        prev_row.clear();
        prev_row.extend(0u32..(width as u32));

        for i in 1..(s_len + 1) {
            let mut row_min = u32::max_value();
            cur_row[0] = i as u32;
            for j in 1..(t_len + 1) {
                let cost = if s_chars[i - 1] == t_chars[j - 1] { 0 } else { 1 };
                let mut current = min(
                    prev_row[j] + 1,           // deletion
                    min(
                        cur_row[j - 1] + 1,    // insertion
                        prev_row[j - 1] + cost // substitution
                    )
                );
                if i > 1 && j > 1 && s_chars[i-1] == t_chars[j-2] && s_chars[i-2] == t_chars[j-1] {
                    current = min(current, prev2_row[j-2] + cost);  // transposition
                }
                if current < row_min {
                    row_min = current;
                }
                cur_row[j] = current;
            }

            let tmp = prev2_row;
            prev2_row = prev_row;
            prev_row = cur_row;
            cur_row = tmp;

            if row_min > max_hint {
                prev_row[t_len] = row_min;
                break;
            }
        }
        out.push(prev_row[t_len]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // until otherwise noted, tests are modifications of tests found in the strsim-rs library,
    // https://github.com/dguo/strsim-rs/blob/ce93ac165200422d21e92879d02f9ac7c7c998bd/src/lib.rs#L533-L614
    // Original tests Copyright 2015 Danny Guo, 2016 Titus Wormer, licensed under the MIT License
    // https://github.com/dguo/strsim-rs/blob/ce93ac165200422d21e92879d02f9ac7c7c998bd/LICENSE
    // Modifications Copyright 2018 Mapbox, made available under same license
    #[test]
    fn mmd_empty() {
        assert_eq!(0, multi_modified_damlev("", &[""])[0]);
    }

    #[test]
    fn mmd_same() {
        assert_eq!(0, multi_modified_damlev("damerau", &["damerau"])[0]);
    }

    #[test]
    fn mmd_first_empty() {
        assert_eq!(7, multi_modified_damlev("", &["damerau"])[0]);
    }

    #[test]
    fn mmd_second_empty() {
        assert_eq!(7, multi_modified_damlev("damerau", &[""])[0]);
    }

    #[test]
    fn mmd_diff() {
        assert_eq!(3, multi_modified_damlev("ca", &["abc"])[0]);
    }

    #[test]
    fn mmd_diff_short() {
        assert_eq!(3, multi_modified_damlev("damerau", &["aderua"])[0]);
    }

    #[test]
    fn mmd_diff_reversed() {
        assert_eq!(3, multi_modified_damlev("aderua", &["damerau"])[0]);
    }

    #[test]
    fn mmd_diff_multibyte() {
        assert_eq!(3, multi_modified_damlev("öঙ香", &["abc"])[0]);
        assert_eq!(3, multi_modified_damlev("abc", &["öঙ香"])[0]);
    }

    #[test]
    fn mmd_diff_unequal_length() {
        assert_eq!(6, multi_modified_damlev("damerau", &["aderuaxyz"])[0]);
    }

    #[test]
    fn mmd_diff_unequal_length_reversed() {
        assert_eq!(6, multi_modified_damlev("aderuaxyz", &["damerau"])[0]);
    }

    #[test]
    fn mmd_diff_comedians() {
        assert_eq!(5, multi_modified_damlev("Stewart", &["Colbert"])[0]);
    }

    #[test]
    fn mmd_many_transpositions() {
        assert_eq!(4, multi_modified_damlev("abcdefghijkl", &["bacedfgihjlk"])[0]);
    }

    #[test]
    fn mmd_diff_longer() {
        let a = "The quick brown fox jumped over the angry dog.";
        let b = "Lehem ipsum dolor sit amet, dicta latine an eam.";
        assert_eq!(36, multi_modified_damlev(a, &[b])[0]);
    }

    #[test]
    fn mmd_beginning_transposition() {
        assert_eq!(1, multi_modified_damlev("foobar", &["ofobar"])[0]);
    }

    #[test]
    fn mmd_end_transposition() {
        assert_eq!(1, multi_modified_damlev("specter", &["spectre"])[0]);
    }

    #[test]
    fn mmd_restricted_edit() {
        assert_eq!(4, multi_modified_damlev("a cat", &["an abct"])[0]);
    }

    // after this point, tests are our own
    #[test]
    fn mmd_multi_dist() {
        assert_eq!(
            vec![0, 1, 2, 3, 6, 7],
            multi_modified_damlev("damerau", &["damerau", "domerau", "domera", "aderua", "aderuaxyz", ""])
        );
    }

    #[test]
    fn mmd_multi_hint() {
        let max_hint = 1;
        let unhinted = multi_modified_damlev("damerau", &["damerau", "domerau", "domera", "aderua", "aderuaxyz", ""]);
        let hinted = multi_modified_damlev_hint("damerau", &["damerau", "domerau", "domera", "aderua", "aderuaxyz", ""], 1);
        for i in 0..unhinted.len() {
            if unhinted[i] <= max_hint {
                assert_eq!(unhinted[i], hinted[i]);
            } else {
                assert!(hinted[i] > max_hint);
            }
        }
    }
}