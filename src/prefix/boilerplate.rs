use std::fmt;
use std::io::prelude::*;
use fst::{IntoStreamer, Streamer};
use fst::raw;
use fst::Error as FstError;
use fst::automaton::{Automaton, AlwaysMatch};

// pretty much everything in this file is copied from either upstream fst::Set or upstream
// fst:Map, so it's quarantined in its own file to separate it from stuff we're actually building
// ourselves (mostly operations relevant to prefixes)

pub struct PrefixSet(raw::Fst);

impl PrefixSet {
    // these are lifted from upstream Set
    #[cfg(feature = "mmap")]
    pub unsafe fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, FstError> {
        raw::Fst::from_path(path).map(PrefixSet)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, FstError> {
        raw::Fst::from_bytes(bytes).map(PrefixSet)
    }

    pub fn from_iter<T, I>(iter: I) -> Result<Self, FstError>
            where T: AsRef<[u8]>, I: IntoIterator<Item=T> {
        let mut builder = PrefixSetBuilder::memory();
        builder.extend_iter(iter)?;
        PrefixSet::from_bytes(builder.into_inner()?)
    }

    pub fn contains<K: AsRef<[u8]>>(&self, key: K) -> bool {
        self.0.contains_key(key)
    }

    pub fn stream(&self) -> Stream {
        Stream(self.0.stream())
    }

    pub fn range(&self) -> StreamBuilder {
        StreamBuilder(self.0.range())
    }

    pub fn search<A: Automaton>(&self, aut: A) -> StreamBuilder<A> {
        StreamBuilder(self.0.search(aut))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_fst(&self) -> &raw::Fst {
        &self.0
    }

    // this one is from Map
    pub fn get<K: AsRef<[u8]>>(&self, key: K) -> Option<u64> {
        self.0.get(key).map(|output| output.value())
    }
}

// Also from Map
impl fmt::Debug for PrefixSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PrefixSet([")?;
        let mut stream = self.stream();
        let mut first = true;
        while let Some((k, v)) = stream.next() {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "({}, {})", String::from_utf8_lossy(k), v)?;
        }
        write!(f, "])")
    }
}

// From Set
impl AsRef<raw::Fst> for PrefixSet {
    fn as_ref(&self) -> &raw::Fst {
        &self.0
    }
}

impl<'s, 'a> IntoStreamer<'a> for &'s PrefixSet {
    type Item = (&'a [u8], u64);
    type Into = Stream<'s>;

    fn into_stream(self) -> Self::Into {
        Stream(self.0.stream())
    }
}

impl From<raw::Fst> for PrefixSet {
    fn from(fst: raw::Fst) -> PrefixSet {
        PrefixSet(fst)
    }
}

pub struct PrefixSetBuilder<W> {
    builder: raw::Builder<W>,
    count: u64
}

impl PrefixSetBuilder<Vec<u8>> {
    pub fn memory() -> Self {
        PrefixSetBuilder { builder: raw::Builder::memory(), count: 0 }
    }
}

impl<W: Write> PrefixSetBuilder<W> {
    pub fn new(wtr: W) -> Result<PrefixSetBuilder<W>, FstError> {
        Ok(PrefixSetBuilder { builder: raw::Builder::new_type(wtr, 0)?, count: 0 })
    }

    pub fn insert<K: AsRef<[u8]>>(&mut self, key: K) -> Result<(), FstError> {
        // this is the main behavior change vs. upstream: enforce autoincrementing IDs
        self.builder.insert(key, self.count)?;
        self.count += 1;
        Ok(())
    }

    pub fn extend_iter<T, I>(&mut self, iter: I) -> Result<(), FstError>
            where T: AsRef<[u8]>, I: IntoIterator<Item=T> {
        for key in iter {
            // likewise, enforce counts
            self.builder.insert(key, self.count)?;
            self.count += 1;
        }
        Ok(())
    }

    pub fn finish(self) -> Result<(), FstError> {
        self.builder.finish()
    }

    pub fn into_inner(self) -> Result<W, FstError> {
        self.builder.into_inner()
    }

    pub fn get_ref(&self) -> &W {
        self.builder.get_ref()
    }

    pub fn bytes_written(&self) -> u64 {
        self.builder.bytes_written()
    }
}

pub struct Stream<'s, A=AlwaysMatch>(raw::Stream<'s, A>) where A: Automaton;

impl<'s, A: Automaton> Stream<'s, A> {
                #[doc(hidden)]
    pub fn new(fst_stream: raw::Stream<'s, A>) -> Self {
        Stream(fst_stream)
    }

    pub fn into_byte_vec(self) -> Vec<(Vec<u8>, u64)> {
        self.0.into_byte_vec()
    }

    pub fn into_str_vec(self) -> Result<Vec<(String, u64)>, FstError> {
        self.0.into_str_vec()
    }

    pub fn into_strs(self) -> Result<Vec<String>, FstError> {
        self.0.into_str_keys()
    }

    pub fn into_bytes(self) -> Vec<Vec<u8>> {
        self.0.into_byte_keys()
    }
}

impl<'a, 's, A: Automaton> Streamer<'a> for Stream<'s, A> {
    type Item = (&'a [u8], u64);

    fn next(&'a mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, out)| (key, out.value()))
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
    type Item = (&'a [u8], u64);
    type Into = Stream<'s, A>;

    fn into_stream(self) -> Self::Into {
        Stream(self.0.into_stream())
    }
}