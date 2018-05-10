use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::error::Error;
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::fs;

#[derive(Default, Debug)]
pub struct FuzzyPhraseSetBuilder {
    phrases: Vec<Vec<u32>>,
    // use a btreemap for this one so we can read them out in order later
    // we'll only have one copy of each word, in the vector, so the inverse
    // map will map from a pointer to an int
    words_to_tmpids: BTreeMap<String, u32>,
    directory: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
struct FuzzyPhraseSetMetadata {
    index_type: String,
    format_version: u32,
}

impl Default for FuzzyPhraseSetMetadata {
    fn default() -> FuzzyPhraseSetMetadata {
        FuzzyPhraseSetMetadata { index_type: "fuzzy_phrase_set".to_string(), format_version: 1 }
    }
}

impl FuzzyPhraseSetBuilder {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Box<Error>> {
        let directory = path.as_ref().to_owned();

        if directory.exists() {
            if !directory.is_dir() {
                return Err(Box::new(IoError::new(IoErrorKind::AlreadyExists, "File exists and is not a directory")));
            }
        } else {
            fs::create_dir(&directory)?;
        }

        Ok(FuzzyPhraseSetBuilder { directory, ..Default::default() })
    }

    pub fn insert(&mut self, phrase: &Vec<&str>) -> Result<(), Box<Error>> {
        let mut tmpid_phrase: Vec<u32> = Vec::with_capacity(phrase.len());
        for word in phrase {
            // the fact that this allocation is necessary even if the string is already in the hashmap is a bummer
            // but absent https://github.com/rust-lang/rfcs/pull/1769 , avoiding it requires a huge amount of hoop-jumping
            let string_word = word.to_string();
            let current_len = self.words_to_tmpids.len();
            let word_id = self.words_to_tmpids.entry(string_word).or_insert(current_len as u32);
            tmpid_phrase.push(word_id.to_owned());
        }
        self.phrases.push(tmpid_phrase);
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), Box<Error>> {
        // we can go from name -> tmpid
        // we need to go from tmpid -> id
        // so build a mapping that does that
        let mut tmpids_to_ids: Vec<u32> = vec![0; self.words_to_tmpids.len()];

        for (id, (_key, tmpid)) in self.words_to_tmpids.iter().enumerate() {
            tmpids_to_ids[*tmpid as usize] = id as u32;
        }

        // next, renumber all of the current phrases with real numbers
        for phrase in self.phrases.iter_mut() {
            for word_idx in (*phrase).iter_mut() {
                *word_idx = tmpids_to_ids[*word_idx as usize];
            }
        }

        self.phrases.sort();

        Ok(())
    }
}