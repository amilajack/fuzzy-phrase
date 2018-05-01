#[derive(Clone)]
pub enum Word {
    Full {
        string: String,
        id: u64,
        edit_distance: i8,
    },
    Prefix {
        string: String,
        // TODO: Change back to range type <30-04-18, boblannon> //
        id_range: (u64, u64),
    },
}

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
    fn two_exact_matches_one_prefix() {

        // two words
        let word_one = Word::Full{ string: String::from("100"), id: 1u64, edit_distance: 0 };
        let word_two = Word::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 0 };
        // last word is a prefix
        let word_three = Word::Prefix{
            string: String::from("st"),
            id_range: (561_528u64, 561_531u64),
        };

        let word_seq = [ &word_one, &word_two, &word_three ];

        let phrase = Phrase::new(&word_seq);

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

    #[test]
    fn two_fuzzy_matches() {

        // two words
        let word_one = Word::Full{ string: String::from("100"), id: 1u64, edit_distance: 1 };
        let word_two = Word::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 2 };

        let word_seq = [ &word_one, &word_two ];

        let phrase = Phrase::new(&word_seq);

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
    fn multiple_combinations() {
        // three words, two variants for third word
        let word_one = Word::Full{ string: String::from("Evergreen"), id: 1u64, edit_distance: 0 };
        let word_two = Word::Full{ string: String::from("Terrace"), id: 61_528u64, edit_distance: 0 };
        let word_three_a = Word::Full{ string: String::from("Springfield"), id: 561_235u64, edit_distance: 0 };
        let word_three_b = Word::Full{ string: String::from("Sprungfeld"), id: 561_247u64, edit_distance: 2 };

        let words_a = [ &word_one, &word_two, &word_three_a ];
        let phrase_a = Phrase::new(&words_a);

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

        let words_b = [ &word_one, &word_two, &word_three_b ];
        let phrase_b = Phrase::new(&words_b);

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
}
