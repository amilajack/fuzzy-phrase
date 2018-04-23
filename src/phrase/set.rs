use std::fmt;
use std::iter::FromIterator;
use std::io;
#[cfg(feature = "mmap")]
use std::path::Path;

use fst::automaton::{Automaton, AlwaysMatch};
use fst::raw;
use fst::{IntoStreamer, Streamer};
use fst::Error as FstError;

use super::util::{three_byte_encode};

pub struct PhraseSet(raw::Fst);

/// PhraseSet is a lexicographically ordered set of phrases.
///
/// Phrases are sequences of words, where each word is represented as an integer. The integers
/// correspond to FuzzyMap values. Due to limitations in the fst library, however, the integers are
/// encoded as a series of 3 bytes.  For example, the three-word phrase "1## Main Street" will be
/// represented over 9 transitions, with one byte each.
///
///     tokens:      1##          main          street
///     integers:    21           457           109821
///     three bytes: [0, 0, 21]   [0, 1, 201]   [1, 172, 253]
///
///
impl PhraseSet {

    #[cfg(feature = "mmap")]
    pub unsafe fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, FstError> {
        raw::Fst::from_path(path).map(PhraseSet)
    }

    /// Create from a raw byte sequence, which must be written by `PhraseSetBuilder`.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, FstError> {
        raw::Fst::from_bytes(bytes).map(PhraseSet)
    }

    /// Convenience function to build a set in memory. Use `PhraseSetBuilder` instead.
    pub fn from_iter<T, I>(iter: I) -> Result<Self, FstError>
            where T: AsRef<[u8]>, I: IntoIterator<Item=T> {
        let mut builder = PhraseSetBuilder::memory();
        builder.extend_iter(iter)?;
        PhraseSet::from_bytes(builder.into_inner()?)
    }

    /// Test membership of a single key
    pub fn contains<K: AsRef<[u8]>>(&self, key: K) -> bool {
        self.0.contains_key(key)
    }

    /// Return a lexicographically ordered stream of all keys in this set.
    pub fn stream(&self) -> Stream {
        Stream(self.0.stream())
    }

    /// Return a builder for range queries.
    pub fn range(&self) -> StreamBuilder {
        StreamBuilder(self.0.range())
    }

    /// Executes an automaton on the keys of this set.
    pub fn search<A: Automaton>(&self, aut: A) -> StreamBuilder<A> {
        StreamBuilder(self.0.search(aut))
    }

    /// Returns the number of elements in this set.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if and only if this set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Creates a new set operation with this set added to it.
    pub fn op(&self) -> OpBuilder {
        OpBuilder::new().add(self)
    }

    /// Returns true if and only if the `self` set is disjoint with the set
    /// `stream`.
    pub fn is_disjoint<'f, I, S>(&self, stream: I) -> bool
            where I: for<'a> IntoStreamer<'a, Into=S, Item=&'a [u8]>,
                  S: 'f + for<'a> Streamer<'a, Item=&'a [u8]> {
        self.0.is_disjoint(StreamZeroOutput(stream.into_stream()))
    }

    /// Returns true if and only if the `self` set is a subset of `stream`.
    pub fn is_subset<'f, I, S>(&self, stream: I) -> bool
            where I: for<'a> IntoStreamer<'a, Into=S, Item=&'a [u8]>,
                  S: 'f + for<'a> Streamer<'a, Item=&'a [u8]> {
        self.0.is_subset(StreamZeroOutput(stream.into_stream()))
    }

    /// Returns true if and only if the `self` set is a superset of `stream`.
    pub fn is_superset<'f, I, S>(&self, stream: I) -> bool
            where I: for<'a> IntoStreamer<'a, Into=S, Item=&'a [u8]>,
                  S: 'f + for<'a> Streamer<'a, Item=&'a [u8]> {
        self.0.is_superset(StreamZeroOutput(stream.into_stream()))
    }

    /// Returns a reference to the underlying raw finite state transducer.
    pub fn as_fst(&self) -> &raw::Fst {
        &self.0
    }

}

impl fmt::Debug for PhraseSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Set([")?;
        let mut stream = self.stream();
        let mut first = true;
        while let Some(key) = stream.next() {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "{}", String::from_utf8_lossy(key))?;
        }
        write!(f, "])")
    }
}



/// Returns the underlying finite state transducer.
impl AsRef<raw::Fst> for PhraseSet {
    fn as_ref(&self) -> &raw::Fst {
        &self.0
    }
}

impl<'s, 'a> IntoStreamer<'a> for &'s PhraseSet {
    type Item = &'a [u8];
    type Into = Stream<'s>;

    fn into_stream(self) -> Self::Into {
        Stream(self.0.stream())
    }
}

// Construct a set from an Fst object.
impl From<raw::Fst> for PhraseSet {
    fn from(fst: raw::Fst) -> PhraseSet {
        PhraseSet(fst)
    }
}

/// A builder for creating a set.
pub struct PhraseSetBuilder<W>(raw::Builder<W>);

impl PhraseSetBuilder<Vec<u8>> {
    /// Create a builder that builds a set in memory.
    pub fn memory() -> Self {
        PhraseSetBuilder(raw::Builder::memory())
    }
}

impl<W: io::Write> PhraseSetBuilder<W> {
    pub fn new(wtr: W) -> Result<PhraseSetBuilder<W>, FstError> {
        raw::Builder::new_type(wtr, 0).map(PhraseSetBuilder)
    }

    /// Insert a phrase, specified as an array of word identifiers.
    pub fn insert(&mut self, phrase: &[u64]) -> Result<(), FstError> {
        let key: Vec<u8> = phrase.into_iter()
                        .flat_map(|word| three_byte_encode(*word))
                        .collect();
        self.0.add(key)
    }

    pub fn extend_iter<T, I>(&mut self, iter: I) -> Result<(), FstError>
            where T: AsRef<[u8]>, I: IntoIterator<Item=T> {
        for key in iter {
            self.0.add(key)?;
        }
        Ok(())
    }

    pub fn extend_stream<'f, I, S>(&mut self, stream: I) -> Result<(), FstError>
            where I: for<'a> IntoStreamer<'a, Into=S, Item=&'a [u8]>,
                  S: 'f + for<'a> Streamer<'a, Item=&'a [u8]> {
        self.0.extend_stream(StreamZeroOutput(stream.into_stream()))
    }

    pub fn finish(self) -> Result<(), FstError> {
        self.0.finish()
    }

    pub fn into_inner(self) -> Result<W, FstError> {
        self.0.into_inner()
    }

    pub fn get_ref(&self) -> &W {
        self.0.get_ref()
    }

    pub fn bytes_written(&self) -> u64 {
        self.0.bytes_written()
    }

}

pub struct Stream<'s, A=AlwaysMatch>(raw::Stream<'s, A>) where A: Automaton;

impl<'s, A: Automaton> Stream<'s, A> {
    #[doc(hidden)]
    pub fn new(fst_stream: raw::Stream<'s, A>) -> Self {
        Stream(fst_stream)
    }

    pub fn into_strs(self) -> Result<Vec<String>, FstError> {
        self.0.into_str_keys()
    }

    pub fn into_bytes(self) -> Vec<Vec<u8>> {
        self.0.into_byte_keys()
    }
}

impl<'a, 's, A: Automaton> Streamer<'a> for Stream<'s, A> {
    type Item = &'a [u8];

    fn next(&'a mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, _)| key)
    }
}

pub struct StreamBuilder<'s, A=AlwaysMatch>(raw::StreamBuilder<'s, A>);

impl<'s, A: Automaton> StreamBuilder<'s, A> {
    pub fn ge<T: AsRef<[u8]>>(self, bound: T) -> Self {
        StreamBuilder(self.0.ge(bound))
    }

    pub fn gt<T: AsRef<[u8]>>(self, bound: T) -> Self {
        StreamBuilder(self.0.gt(bound))
    }

    pub fn le<T: AsRef<[u8]>>(self, bound: T) -> Self {
        StreamBuilder(self.0.le(bound))
    }

    pub fn lt<T: AsRef<[u8]>>(self, bound: T) -> Self {
        StreamBuilder(self.0.lt(bound))
    }
}

impl<'s, 'a, A: Automaton> IntoStreamer<'a> for StreamBuilder<'s, A> {
    type Item = &'a [u8];
    type Into = Stream<'s, A>;

    fn into_stream(self) -> Self::Into {
        Stream(self.0.into_stream())
    }
}

pub struct OpBuilder<'s>(raw::OpBuilder<'s>);

impl<'s> OpBuilder<'s> {
    pub fn new() -> Self {
        OpBuilder(raw::OpBuilder::new())
    }

    pub fn add<I, S>(mut self, streamable: I) -> Self
            where I: for<'a> IntoStreamer<'a, Into=S, Item=&'a [u8]>,
                  S: 's + for<'a> Streamer<'a, Item=&'a [u8]> {
        self.push(streamable);
        self
    }

    pub fn push<I, S>(&mut self, streamable: I)
            where I: for<'a> IntoStreamer<'a, Into=S, Item=&'a [u8]>,
                  S: 's + for<'a> Streamer<'a, Item=&'a [u8]> {
        self.0.push(StreamZeroOutput(streamable.into_stream()));
    }

    pub fn union(self) -> Union<'s> {
        Union(self.0.union())
    }

    pub fn intersection(self) -> Intersection<'s> {
        Intersection(self.0.intersection())
    }

    pub fn difference(self) -> Difference<'s> {
        Difference(self.0.difference())
    }

    pub fn symmetric_difference(self) -> SymmetricDifference<'s> {
        SymmetricDifference(self.0.symmetric_difference())
    }
}

impl<'f, I, S> Extend<I> for OpBuilder<'f>
    where I: for<'a> IntoStreamer<'a, Into=S, Item=&'a [u8]>,
          S: 'f + for<'a> Streamer<'a, Item=&'a [u8]> {
    fn extend<T>(&mut self, it: T) where T: IntoIterator<Item=I> {
        for stream in it {
            self.push(stream);
        }
    }
}

impl<'f, I, S> FromIterator<I> for OpBuilder<'f>
    where I: for<'a> IntoStreamer<'a, Into=S, Item=&'a [u8]>,
          S: 'f + for<'a> Streamer<'a, Item=&'a [u8]> {
    fn from_iter<T>(it: T) -> Self where T: IntoIterator<Item=I> {
        let mut op = OpBuilder::new();
        op.extend(it);
        op
    }
}

pub struct Union<'s>(raw::Union<'s>);

impl<'a, 's> Streamer<'a> for Union<'s> {
    type Item = &'a [u8];

    fn next(&'a mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, _)| key)
    }
}

pub struct Intersection<'s>(raw::Intersection<'s>);

impl<'a, 's> Streamer<'a> for Intersection<'s> {
    type Item = &'a [u8];

    fn next(&'a mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, _)| key)
    }
}

pub struct Difference<'s>(raw::Difference<'s>);

impl<'a, 's> Streamer<'a> for Difference<'s> {
    type Item = &'a [u8];

    fn next(&'a mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, _)| key)
    }
}

pub struct SymmetricDifference<'s>(raw::SymmetricDifference<'s>);

impl<'a, 's> Streamer<'a> for SymmetricDifference<'s> {
    type Item = &'a [u8];

    fn next(&'a mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, _)| key)
    }
}

struct StreamZeroOutput<S>(S);

impl<'a, S: Streamer<'a>> Streamer<'a> for StreamZeroOutput<S> {
    type Item = (S::Item, raw::Output);

    fn next(&'a mut self) -> Option<Self::Item> {
        self.0.next().map(|key| (key, raw::Output::zero()))
    }
}

