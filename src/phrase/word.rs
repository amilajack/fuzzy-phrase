#[derive(Clone)]
pub enum Word {
    Full {
        string: String,
        id: u64,
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
    edit_distances: &'a[i32],
}

impl<'a> Phrase<'a> {
    pub fn new(words: &'a[&'a Word], edit_distances: &'a [i32]) -> Phrase<'a> {
        let length: usize = words.len();
        Phrase {
            words,
            length,
            edit_distances,
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
    total_distance: i32,
}

impl<'a> Iterator for PhraseIterator<'a> {
    type Item = &'a Word;

    fn next(&mut self) -> Option<&'a Word> {
        if self.offset >= self.phrase.length {
            return None
        } else {
            let word = self.phrase.words[self.offset];
            self.total_distance += self.phrase.edit_distances[self.offset];
            self.offset += 1;
            Some(word)
        }
    }
}

impl<'a> IntoIterator for &'a Phrase<'a> {
    type Item = &'a Word;
    type IntoIter = PhraseIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        PhraseIterator{ phrase: self, offset: 0, total_distance: 0}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_exact_matches_one_prefix() {

        // two words
        let word_one = Word::Full{ string: String::from("100"), id: 1u64 };
        let word_two = Word::Full{ string: String::from("main"), id: 61_528u64 };
        // last word is a prefix
        let word_three = Word::Prefix{
            string: String::from("st"),
            id_range: (561_528u64, 561_531u64),
        };

        let word_seq = [
            &word_one,
            &word_two,
            &word_three
        ];

        let edit_distances = [ 0, 0, 0 ];
        let phrase = Phrase::new(&word_seq, &edit_distances);

        let mut word_count = 0;
        let mut word_ids = vec![];

        let mut prefix_count = 0;
        let mut prefix_ids = vec![];

        let phrase_iter = phrase.into_iter();

        for ref word in phrase_iter {
            match word {
                &&Word::Full{ ref string,  ref id } => {
                    word_count += 1;
                    word_ids.push(*id);
                },
                &&Word::Prefix{ ref string, ref id_range } => {
                    prefix_count += 1;
                    for i in (*id_range).0..(*id_range).1 {
                        prefix_ids.push(i);
                    }
                }
            }
        }
        assert_eq!(0, phrase_iter.total_distance);

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
        let word_one = Word::Full{ string: String::from("100"), id: 1u64 };
        let word_two = Word::Full{ string: String::from("main"), id: 61_528u64 };

        let word_seq = [
            &word_one,
            &word_two,
        ];

        let edit_distances = [ 1, 2 ];
        let phrase = Phrase::new(&word_seq, &edit_distances);

        let mut word_count = 0;
        let mut word_ids = vec![];

        let phrase_iter = phrase.into_iter();

        for (i, ref word) in phrase_iter.enumerate() {
            assert_eq!(edit_distances[i], phrase_iter.total_distance);
            match word {
                &&Word::Full{ ref string,  ref id } => {
                    word_count += 1;
                    word_ids.push(*id);
                },
                _ => {
                    panic!("Should be all full words");
                }
            }
        }
        assert_eq!(3, phrase_iter.total_distance);

        // should be 2 full words, 2 ids
        assert_eq!(2, word_count);
        assert_eq!(vec![1, 61_528], word_ids);

    }
}
