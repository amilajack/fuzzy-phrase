use std::ops::Range;

#[derive(Clone)]
pub enum Word {
    Full {
        string: String,
        id: u64,
        edit_distance: u64,
    },
    Prefix {
        string: String,
        id_range: Range<u64>,
        // not sure this makes sense
        // edit_distance: u64,
    },
}

pub struct Phrase {
    length: usize,
    words: Vec<Word>,
}

impl Phrase {
    pub fn new(words: &[Word]) -> Phrase {
        let length: usize = words.len();
        Phrase {
            words: words.to_vec(),
            length: length,
        }
    }

    pub fn len(&self) -> usize {
        self.length
    }
}

impl IntoIterator for Phrase {
    type Item = Word;
    type IntoIter = ::std::vec::IntoIter<Word>;

    fn into_iter(self) -> Self::IntoIter {
        self.words.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_exact_matches_one_prefix() {
        let word_seq = [
            // two words
            Word::Full{ string: String::from("100") , id: 1u64,      edit_distance: 0 },
            Word::Full{ string: String::from("main"), id: 61_528u64, edit_distance: 0 },
            // last word is a prefix
            Word::Prefix{
                string: String::from("st"),
                id_range: Range { start: 561_528u64, end: 561_531u64 }
            },
        ];
        let phrase = Phrase::new(&word_seq);

        let mut word_count = 0;
        let mut word_ids = vec![];

        let mut prefix_count = 0;
        let mut prefix_ids = vec![];

        for word in phrase.into_iter() {
            match word {
                Word::Full{ string, id, edit_distance } => {
                    word_count += 1;
                    word_ids.push(id);
                },
                Word::Prefix{ string, id_range } => {
                    prefix_count += 1;
                    for i in id_range {
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
}
