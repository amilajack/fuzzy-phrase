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
/// This implementation is inspired pretty directly by the pseudocode here:
/// https://en.wikipedia.org/wiki/Damerau%E2%80%93Levenshtein_distance#Optimal_string_alignment_distance
/// plus some alterations to avoid work repetition when comparing the same target string to
/// multiple different candidate matches, as we do in the context of Symspell lookups. It avoids
/// repeating Unicode parses if possible, and also reuses the main distance matrix across multiple
/// sets of comparisons. This distance matrix is a ~len(a)*len(b) matrix, modeled in, e.g.,
/// simstring, as a vector of vectors, but here we emulate a 2D matrix using a single linear vector
/// to improve spatial locality and reduce allocations. Further, we can see that matrix is
/// initially constructed with 0..len(a) along the top row and 0..len(b) along the first column,
/// and these values never change over the course of the run of the algorithm, and further that the
/// remaining cells are filled in in order from top left to bottom right, looking only at
/// already-filled in cells to do so. Finally, noßthing bad happens if the matrix is oversized; we
/// just don't end up consulting some rows. This means if we're doing multiple matches, we can
/// construct a single vector up front that's big enough for the biggest word (so, max candidate
/// length * target length) and populate the first row and (simulated) first column, and then reuse
/// it for all of the words we're checking.

pub fn multi_modified_damlev<T: AsRef<str>>(target: T, sources: &[T]) -> Vec<u32> {
    let t_chars: Vec<char> = target.as_ref().chars().collect();
    let t_len = t_chars.len();

    if t_len == 0 {
        return sources.iter().map(|s| s.as_ref().chars().count() as u32).collect();
    }

    let mut max_s_len = 0;
    let s_count = sources.len();
    let mut s_char_vec: Vec<Vec<char>> = Vec::with_capacity(s_count);
    for s in sources {
        let s_chars: Vec<char> = s.as_ref().chars().collect();
        let s_len = s_chars.len();
        if s_len > max_s_len {
            max_s_len = s_len;
        }
        s_char_vec.push(s_chars);
    }

    let d_width = t_len + 1;
    let d_height = max_s_len + 1;
    let mut d: Vec<u32> = vec![0; d_width * d_height];
    // we're going to want to be able to pretend to do lookups like d[x][y] even though d is
    // actually 1-dimensional, so this is a handly closure to do that
    let idx = |x, y| x + (y * d_width);

    for i in 0..=t_len {
        // we're conceptually setting d[i,0] but that's equivalent to d[i]
        d[i] = i as u32;
    }
    for j in 0..=max_s_len {
        // conceptually d[0,j] but we'll skip the useless addition
        d[j*d_width] = j as u32;
    }

    let mut out: Vec<u32> = Vec::with_capacity(s_count);
    for s_chars in s_char_vec {
        let s_len = s_chars.len();

        if t_chars == s_chars {
            out.push(0);
            continue;
        } else if s_len == 0 {
            out.push(t_len as u32);
            continue;
        }

        for i in 1..=t_len {
            for j in 1..=s_len {
                let cost = if t_chars[i - 1] == s_chars[j - 1] { 0 } else { 1 };
                d[idx(i, j)] = min(
                    d[idx(i-1, j)] + 1,         // deletion
                    min(
                        d[idx(i, j-1)] + 1,     // insertion
                        d[idx(i-1, j-1)] + cost // substitution
                    )
                );
                if i > 1 && j > 1 && t_chars[i-1] == s_chars[j-2] && t_chars[i-2] == s_chars[j-1] {
                    d[idx(i, j)] = min(d[idx(i, j)], d[idx(i-2, j-2)] + cost);  // transposition
                }
            }
        }
        out.push(d[idx(t_len, s_len)]);
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
}