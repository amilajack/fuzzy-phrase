use super::util;

/// An abstraction over full words and prefixes.
#[derive(Clone)]
pub enum QueryWord {
    /// A `Full` word is a word that has an identifier and is one of the members of a PrefixSet.
    Full {
        string: String,
        id: u64,
        edit_distance: i8,
    },

    /// A `Prefix` is a string that is the prefix to more than one full word, and includes an id_range field,
    /// which of identifiers.
    Prefix {
        string: String,
        id_range: (u64, u64),
    },
}

/// A specialized container for a sequence of `QueryWord`s.
///
/// It allows iterating over a sequence of `QueryWord`s without taking ownership of them.  the `words`
/// field contains a _slice_ of _references_ to `QueryWord`s. This is helpful design because we'll often
/// be combining and re-combining the elements of multiple `Vec<QueryWord>`s. See the
/// `multiple_combinations` test below for an example. It's important that we work on references to
/// the elements of those arrays so that they can be re-used.
///
/// Because the `words` fields is made up of pointers, the lifetime annotations are necessary to
/// make sure that the sequence of words used to make the phrase does not go out of scope.
pub struct QueryPhrase<'a> {
    pub length: usize,
    pub words: &'a[&'a QueryWord],
    pub has_prefix: bool,
}

impl<'a> QueryPhrase<'a> {
    pub fn new(words: &'a[&'a QueryWord]) -> QueryPhrase<'a> {
        let length: usize = words.len();
        let has_prefix: bool = match words[length - 1] {
            &QueryWord::Full {..} => false,
            &QueryWord::Prefix {..} => true,
        };
        // disallow prefixes in any position except the final position
        for i in 0..length-1 {
            match words[i] {
                &QueryWord::Prefix {..} => {
                    panic!("Non-terminal QueryWord::Prefix found");
                },
                _ => ()
            }
        }

        QueryPhrase {
            words,
            length,
            has_prefix,
        }
    }

    /// Return the length of the phrase (number of words)
    pub fn len(&self) -> usize {
        self.length
    }

    /// Sum the edit distances of the full words in the phrase
    pub fn total_edit_distance(&self) -> i8 {
        let mut total_edit_distance = 0;
        for word in self.words {
            match word {
                &&QueryWord::Full{ ref edit_distance, .. } => {
                    total_edit_distance += *edit_distance;
                },
                _ => (),
            }
        }
        total_edit_distance
    }

    /// Generate a key from the ids of the full words in this phrase
    pub fn full_word_key(&self) -> Vec<u8> {
        let mut word_ids: Vec<u64> = vec![];
        for word in self.words {
            match word {
                &&QueryWord::Full{ ref id, .. } => {
                    word_ids.push(*id);
                },
                _ => (),
            }
        }
        util::word_ids_to_key(&word_ids)
    }

    /// Generate a key from the prefix range
    pub fn prefix_key_range(&self) -> Option<(Vec<u8>, Vec<u8>)> {
        let prefix_range = match self.words[self.length - 1] {
            &QueryWord::Prefix{ ref id_range, .. } => {
                *id_range
            },
            _ => return None,
        };
        let prefix_start_key = util::three_byte_encode(prefix_range.0);
        let prefix_end_key = util::three_byte_encode(prefix_range.1);

        Some((prefix_start_key, prefix_end_key))
    }

}

#[derive(Copy, Clone)]
pub struct QueryPhraseIterator<'a> {
    phrase: &'a QueryPhrase<'a>,
    offset: usize,
}

impl<'a> Iterator for QueryPhraseIterator<'a> {
    type Item = &'a QueryWord;

    fn next(&mut self) -> Option<&'a QueryWord> {
        if self.offset >= self.phrase.length {
            return None
        } else {
            let ref word = self.phrase.words[self.offset];
            self.offset += 1;
            Some(word)
        }
    }
}

impl<'a> IntoIterator for &'a QueryPhrase<'a> {
    type Item = &'a QueryWord;
    type IntoIter = QueryPhraseIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        QueryPhraseIterator{ phrase: self, offset: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_from_words() {
        let words = vec![
            vec![ QueryWord::Full{ string: String::from("100"), id: 1u64, edit_distance: 0 } ],
            vec![ QueryWord::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 0 } ],
            vec![ QueryWord::Full{ string: String::from("st"), id: 561_528u64, edit_distance: 0 } ],
        ];

        let word_seq = [ &words[0][0], &words[1][0], &words[2][0] ];

        let phrase = QueryPhrase::new(&word_seq[..]);
        assert_eq!(3, phrase.len());
        assert_eq!(false, phrase.has_prefix);
        assert_eq!(
            vec![
                0u8, 0u8,   1u8,     // 1
                0u8, 240u8, 88u8,    // 61_528
                8u8, 145u8, 120u8,   // 561_528
            ],
            phrase.full_word_key()
        );

        let shingle_one = QueryPhrase::new(&word_seq[0..2]);
        assert_eq!(2, shingle_one.len());

        let shingle_two = QueryPhrase::new(&word_seq[1..3]);
        assert_eq!(2, shingle_two.len());

    }

    #[test]
    fn phrase_multiple_combinations() {
        // three words, two variants for third word
        let words = vec![
            vec![ QueryWord::Full{ string: String::from("Evergreen"), id: 1u64, edit_distance: 0 } ],
            vec![ QueryWord::Full{ string: String::from("Terrace"), id: 61_528u64, edit_distance: 0 } ],
            vec![
                QueryWord::Full{ string: String::from("Springfield"), id: 561_235u64, edit_distance: 0 },
                QueryWord::Full{ string: String::from("Sprungfeld"), id: 561_247u64, edit_distance: 2 },
            ],
        ];

        let word_seq_a = [ &words[0][0], &words[1][0], &words[2][0] ];
        let phrase_a = QueryPhrase::new(&word_seq_a);

        assert_eq!(0, phrase_a.total_edit_distance());
        assert_eq!(false, phrase_a.has_prefix);
        assert_eq!(
            vec![
                0u8, 0u8,   1u8,    // 1
                0u8, 240u8, 88u8,   // 61_528
                8u8, 144u8, 83u8,   // 561_235
            ],
            phrase_a.full_word_key()
        );

        let mut word_ids = vec![];

        for word in phrase_a.into_iter() {
            match word {
                &QueryWord::Full{ ref id, .. } => {
                    word_ids.push(*id);
                },
                _ => {
                    panic!("Should be all full words");
                }
            }
        }

        // should be 2 full words, 2 ids
        assert_eq!(vec![1u64, 61_528u64, 561_235u64], word_ids);

        let word_seq_b = [ &words[0][0], &words[1][0], &words[2][1] ];
        let phrase_b = QueryPhrase::new(&word_seq_b);
        assert_eq!(2, phrase_b.total_edit_distance());
        assert_eq!(false, phrase_b.has_prefix);
        assert_eq!(
            vec![
                0u8, 0u8,   1u8,    // 1
                0u8, 240u8, 88u8,   // 61_528
                8u8, 144u8, 95u8,   // 561_247
            ],
            phrase_b.full_word_key()
        );

        let mut word_ids = vec![];

        for word in phrase_b.into_iter() {
            match word {
                &QueryWord::Full{ ref id, .. } => {
                    word_ids.push(*id);
                },
                _ => {
                    panic!("Should be all full words");
                }
            }
        }

        // should be 2 full words, 2 ids
        assert_eq!(vec![1u64, 61_528u64, 561_247u64], word_ids);
    }

    #[test]
    fn two_fuzzy_matches() {

        let words = vec![
            vec![ QueryWord::Full{ string: String::from("100"), id: 1u64, edit_distance: 1 } ],
            vec![ QueryWord::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 2 } ],
        ];
        let word_seq = [ &words[0][0], &words[1][0] ];
        let phrase = QueryPhrase::new(&word_seq[..]);

        assert_eq!(3, phrase.total_edit_distance());
        assert_eq!(false, phrase.has_prefix);
        assert_eq!(
            vec![
                0u8, 0u8,   1u8,     // 1
                0u8, 240u8, 88u8,    // 61_528
            ],
            phrase.full_word_key()
        );
        assert_eq!(None, phrase.prefix_key_range());

        let mut word_count = 0;
        let mut word_ids = vec![];

        let phrase_iter = phrase.into_iter();

        for word in phrase_iter {
            match word {
                &QueryWord::Full{ ref id, .. } => {
                    word_count += 1;
                    word_ids.push(*id);
                },
                _ => {
                    panic!("Should be all full words");
                }
            }
        }

        // should be 2 full words, 2 ids
        assert_eq!(2, word_count);
        assert_eq!(vec![1, 61_528], word_ids);

    }


    #[test]
    fn two_exact_matches_one_prefix() {

        let words = vec![
            vec![ QueryWord::Full{ string: String::from("100"), id: 1u64, edit_distance: 0 } ],
            vec![ QueryWord::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 0 } ],
            vec![ QueryWord::Prefix{ string: String::from("st"), id_range: (561_528u64, 561_531u64) } ],
        ];
        let word_seq = [ &words[0][0], &words[1][0], &words[2][0] ];
        let phrase = QueryPhrase::new(&word_seq[..]);

        assert_eq!(0, phrase.total_edit_distance());
        assert_eq!(true, phrase.has_prefix);
        assert_eq!(
            vec![
                0u8, 0u8,   1u8,     // 1
                0u8, 240u8, 88u8,    // 61_528
            ],
            phrase.full_word_key()
        );

        assert_eq!(
            Some((
                vec![ 8u8, 145u8, 120u8],     // 561_528
                vec![ 8u8, 145u8, 123u8],     // 561_531
            )),
            phrase.prefix_key_range()
        );

        let mut word_count = 0;
        let mut word_ids = vec![];

        let mut prefix_count = 0;
        let mut prefix_ids = vec![];

        let phrase_iter = phrase.into_iter();

        for word in phrase_iter {
            match word {
                &QueryWord::Full{ ref id, .. } => {
                    word_count += 1;
                    word_ids.push(*id);
                },
                &QueryWord::Prefix{ ref id_range, .. } => {
                    prefix_count += 1;
                    for i in (*id_range).0..(*id_range).1 {
                        prefix_ids.push(i);
                    }
                }
            }
        }

        // should be 2 full words, 2 ids
        assert_eq!(2, word_count);
        assert_eq!(vec![1, 61_528], word_ids);

        // should be 1 prefix, 3 ids
        assert_eq!(1, prefix_count);
        assert_eq!(vec![561_528, 561_529, 561_530], prefix_ids);
    }

    #[test]
    #[should_panic]
    fn non_terminal_prefix() {

        let words = vec![
            vec![ QueryWord::Full{ string: String::from("100"), id: 1u64, edit_distance: 0 } ],
            vec![ QueryWord::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 0 } ],
            vec![ QueryWord::Prefix{ string: String::from("st"), id_range: (561_528u64, 561_531u64) } ],
        ];
        let word_seq = [ &words[0][0], &words[2][0], &words[1][0] ];
        QueryPhrase::new(&word_seq[..]);
    }

}
