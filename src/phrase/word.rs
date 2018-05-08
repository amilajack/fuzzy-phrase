#[derive(Clone)]
pub enum Word {
    Full {
        string: String,
        id: u64,
        edit_distance: i8,
    },
    Prefix {
        string: String,
        id_range: (u64, u64),
    },
}

// TODO: explaining what everything's purpose is, and why we need lifetimes <01-05-18, boblannon> //
/// A specialized container for a sequence of `Word`s.
///
/// It allows iterating over a sequence of `Word`s without taking ownership of them.  the `words`
/// field contains a _slice_ of _references_ to `Word`s. This is helpful design because we'll often
/// be combining and re-combining the elements of multiple `Vec<Word>`s. See the
/// `multiple_combinations` test below for an example. It's important that we work on references to
/// the elements of those arrays so that they can be re-used.
///
/// The lifetime annotations are necessary
pub struct Phrase<'a> {
    length: usize,
    words: &'a[&'a Word],
}

impl<'a> Phrase<'a> {
    pub fn new(words: &'a[&'a Word]) -> Phrase<'a> {
        let length: usize = words.len();
        Phrase {
            words,
            length,
        }
    }

    pub fn len(&self) -> usize {
        self.length
    }

}

#[derive(Copy, Clone)]
pub struct PhraseIterator<'a> {
    phrase: &'a Phrase<'a>,
    offset: usize,
}

impl<'a> Iterator for PhraseIterator<'a> {
    type Item = &'a Word;

    fn next(&mut self) -> Option<&'a Word> {
        if self.offset >= self.phrase.length {
            return None
        } else {
            let ref word = self.phrase.words[self.offset];
            self.offset += 1;
            Some(word)
        }
    }
}

impl<'a> IntoIterator for &'a Phrase<'a> {
    type Item = &'a Word;
    type IntoIter = PhraseIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        PhraseIterator{ phrase: self, offset: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_from_words() {
        let words = vec![
            vec![ Word::Full{ string: String::from("100"), id: 1u64, edit_distance: 0 } ],
            vec![ Word::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 0 } ],
            vec![ Word::Full{ string: String::from("st"), id: 561_528u64, edit_distance: 0 } ],
        ];

        let word_seq = [ &words[0][0], &words[1][0], &words[2][0] ];

        let phrase = Phrase::new(&word_seq[..]);
        assert_eq!(3, phrase.len());

        let shingle_one = Phrase::new(&word_seq[0..2]);
        assert_eq!(2, shingle_one.len());

        let shingle_two = Phrase::new(&word_seq[1..3]);
        assert_eq!(2, shingle_two.len());

    }

    #[test]
    fn phrase_multiple_combinations() {
        // three words, two variants for third word
        let words = vec![
            vec![ Word::Full{ string: String::from("Evergreen"), id: 1u64, edit_distance: 0 } ],
            vec![ Word::Full{ string: String::from("Terrace"), id: 61_528u64, edit_distance: 0 } ],
            vec![
                Word::Full{ string: String::from("Springfield"), id: 561_235u64, edit_distance: 0 },
                Word::Full{ string: String::from("Sprungfeld"), id: 561_247u64, edit_distance: 2 },
            ],
        ];

        let word_seq_a = [ &words[0][0], &words[1][0], &words[2][0] ];
        let phrase_a = Phrase::new(&word_seq_a);

        let mut word_ids = vec![];
        let mut total_distance = 0;

        for word in phrase_a.into_iter() {
            match word {
                &Word::Full{ ref string,  ref id, ref edit_distance } => {
                    word_ids.push(*id);
                    total_distance += edit_distance;
                },
                _ => {
                    panic!("Should be all full words");
                }
            }
        }
        assert_eq!(0, total_distance);

        // should be 2 full words, 2 ids
        assert_eq!(vec![1u64, 61_528u64, 561_235u64], word_ids);

        let word_seq_b = [ &words[0][0], &words[1][0], &words[2][1] ];
        let phrase_b = Phrase::new(&word_seq_b);

        let mut word_ids = vec![];
        let mut total_distance = 0;

        for word in phrase_b.into_iter() {
            match word {
                &Word::Full{ ref string,  ref id, ref edit_distance } => {
                    word_ids.push(*id);
                    total_distance += edit_distance;
                },
                _ => {
                    panic!("Should be all full words");
                }
            }
        }
        assert_eq!(2, total_distance);

        // should be 2 full words, 2 ids
        assert_eq!(vec![1u64, 61_528u64, 561_247u64], word_ids);
    }

    #[test]
    fn two_fuzzy_matches() {

        let words = vec![
            vec![ Word::Full{ string: String::from("100"), id: 1u64, edit_distance: 1 } ],
            vec![ Word::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 2 } ],
        ];
        let word_seq = [ &words[0][0], &words[1][0] ];
        let phrase = Phrase::new(&word_seq[..]);

        let mut word_count = 0;
        let mut word_ids = vec![];
        let mut total_distance = 0;

        let phrase_iter = phrase.into_iter();

        for word in phrase_iter {
            match word {
                &Word::Full{ ref string,  ref id, ref edit_distance } => {
                    word_count += 1;
                    total_distance += edit_distance;
                    word_ids.push(*id);
                },
                _ => {
                    panic!("Should be all full words");
                }
            }
        }
        assert_eq!(3, total_distance);

        // should be 2 full words, 2 ids
        assert_eq!(2, word_count);
        assert_eq!(vec![1, 61_528], word_ids);

    }


    #[test]
    fn two_exact_matches_one_prefix() {

        let words = vec![
            vec![ Word::Full{ string: String::from("100"), id: 1u64, edit_distance: 0 } ],
            vec![ Word::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 0 } ],
            vec![ Word::Prefix{ string: String::from("st"), id_range: (561_528u64, 561_531u64) } ],
        ];
        let word_seq = [ &words[0][0], &words[1][0], &words[2][0] ];
        let phrase = Phrase::new(&word_seq[..]);

        let mut total_distance = 0;
        let mut word_count = 0;
        let mut word_ids = vec![];

        let mut prefix_count = 0;
        let mut prefix_ids = vec![];

        let phrase_iter = phrase.into_iter();

        for word in phrase_iter {
            match word {
                &Word::Full{ ref string,  ref id, ref edit_distance } => {
                    word_count += 1;
                    total_distance += edit_distance;
                    word_ids.push(*id);
                },
                &Word::Prefix{ ref string, ref id_range } => {
                    prefix_count += 1;
                    for i in (*id_range).0..(*id_range).1 {
                        prefix_ids.push(i);
                    }
                }
            }
        }
        assert_eq!(0, total_distance);

        // should be 2 full words, 2 ids
        assert_eq!(2, word_count);
        assert_eq!(vec![1, 61_528], word_ids);

        // should be 1 prefix, 3 ids
        assert_eq!(1, prefix_count);
        assert_eq!(vec![561_528, 561_529, 561_530], prefix_ids);
    }

}
