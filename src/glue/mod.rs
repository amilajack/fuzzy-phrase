use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::error::Error;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, BufReader, BufWriter};
use std::fs;

use serde_json;

use ::prefix::{PrefixSet, PrefixSetBuilder};
use ::phrase::{PhraseSet, PhraseSetBuilder};
use ::phrase::query::{QueryPhrase, QueryWord};

#[derive(Default, Debug)]
pub struct FuzzyPhraseSetBuilder {
    phrases: Vec<Vec<u32>>,
    // use a btreemap for this one so we can read them out in order later
    // we'll only have one copy of each word, in the vector, so the inverse
    // map will map from a pointer to an int
    words_to_tmpids: BTreeMap<String, u32>,
    directory: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
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

    pub fn insert(&mut self, phrase: &[&str]) -> Result<(), Box<Error>> {
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

        let prefix_writer = BufWriter::new(fs::File::create(self.directory.join(Path::new("prefix.fst")))?);
        let mut prefix_set_builder = PrefixSetBuilder::new(prefix_writer)?;

        // words_to_tmpids is a btreemap over word keys,
        // so when we iterate over it, we'll get back words sorted
        // we'll do three things with that:
        // - build up our prefix set
        // - map from temporary IDs to lex ids (which we can get just be enumerating our sorted list)
        // - build up our fuzzy set (this one doesn't require the sorted words, but it doesn't hurt)
        for (id, (word, tmpid)) in self.words_to_tmpids.iter().enumerate() {
            prefix_set_builder.insert(word)?;

            tmpids_to_ids[*tmpid as usize] = id as u32;

            // TODO: insert into fuzzy map
        }

        prefix_set_builder.finish()?;

        // next, renumber all of the current phrases with real rather than temp IDs
        for phrase in self.phrases.iter_mut() {
            for word_idx in (*phrase).iter_mut() {
                *word_idx = tmpids_to_ids[*word_idx as usize];
            }
        }

        self.phrases.sort();

        let phrase_writer = BufWriter::new(fs::File::create(self.directory.join(Path::new("phrase.fst")))?);
        let mut phrase_set_builder = PhraseSetBuilder::new(phrase_writer)?;

        for phrase in self.phrases {
            phrase_set_builder.insert(&phrase)?;
        }

        phrase_set_builder.finish()?;

        let metadata = FuzzyPhraseSetMetadata::default();
        let metadata_writer = BufWriter::new(fs::File::create(self.directory.join(Path::new("metadata.json")))?);
        serde_json::to_writer_pretty(metadata_writer, &metadata)?;

        Ok(())
    }
}

pub struct FuzzyPhraseSet {
    prefix_set: PrefixSet,
    phrase_set: PhraseSet,
}

impl FuzzyPhraseSet {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Box<Error>> {
        let directory = path.as_ref();

        if !directory.exists() || !directory.is_dir() {
            return Err(Box::new(IoError::new(IoErrorKind::NotFound, "File does not exist or is not a directory")));
        }

        let metadata_reader = BufReader::new(fs::File::open(directory.join(Path::new("metadata.json")))?);
        let metadata: FuzzyPhraseSetMetadata = serde_json::from_reader(metadata_reader)?;
        if metadata != FuzzyPhraseSetMetadata::default() {
            return Err(Box::new(IoError::new(IoErrorKind::InvalidData, "Unexpected structure metadata")));
        }

        let prefix_path = directory.join(Path::new("prefix.fst"));
        if !prefix_path.exists() {
            return Err(Box::new(IoError::new(IoErrorKind::NotFound, "Prefix FST does not exist")));
        }
        let prefix_set = unsafe { PrefixSet::from_path(&prefix_path) }?;

        let phrase_path = directory.join(Path::new("phrase.fst"));
        if !phrase_path.exists() {
            return Err(Box::new(IoError::new(IoErrorKind::NotFound, "Phrase FST does not exist")));
        }
        let phrase_set = unsafe { PhraseSet::from_path(&phrase_path) }?;

        Ok(FuzzyPhraseSet { prefix_set, phrase_set })
    }

    pub fn contains(&self, phrase: &Vec<&str>) -> Result<bool, Box<Error>> {
        let mut id_phrase: Vec<QueryWord> = Vec::with_capacity(phrase.len());
        for word in phrase {
            match self.prefix_set.get(&word) {
                Some(word_id) => { id_phrase.push(QueryWord::Full { id: word_id as u32, edit_distance: 0 }) },
                None => { return Ok(false) }
            }
        }
        Ok(self.phrase_set.contains(QueryPhrase::new(&id_phrase)?)?)
    }

    pub fn contains_prefix(&self, phrase: &[&str]) -> Result<bool, Box<Error>> {
        let mut id_phrase: Vec<QueryWord> = Vec::with_capacity(phrase.len());
        if phrase.len() > 0 {
            let last_idx = phrase.len() - 1;
            for word in phrase[..last_idx].iter() {
                match self.prefix_set.get(&word) {
                    Some(word_id) => { id_phrase.push(QueryWord::Full { id: word_id as u32, edit_distance: 0 }) },
                    None => { return Ok(false) }
                }
            }
            match self.prefix_set.get_prefix_range(&phrase[last_idx]) {
                Some((word_id_start, word_id_end)) => { id_phrase.push(QueryWord::Prefix { id_range: (word_id_start.value() as u32, word_id_end.value() as u32) }) },
                None => { return Ok(false) }
            }
        }
        Ok(self.phrase_set.contains_prefix(QueryPhrase::new(&id_phrase)?)?)
    }
}