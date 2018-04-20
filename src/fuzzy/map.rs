use fst::{IntoStreamer, Streamer, Set, Map, MapBuilder, Automaton};
use std::io::prelude::*;
use fst::raw;
use fst::Error as FstError;
use fst::automaton::{AlwaysMatch};


pub struct FuzzySetBuilder<W> {
    builder: raw::Builder<W>
}

impl FuzzySetBuilder<Vec<u8>> {
    pub fn memory() -> Self {
        FuzzySetBuilder { builder: raw::Builder::memory() }
    }
}

impl<W: Write> FuzzySetBuilder<W> {
    pub fn new(fst_wtr: W) -> Result<FuzzySetBuilder<W>, FstError> {
        Ok(FuzzySetBuilder { builder: raw::Builder::new_type(fst_wtr, 0)? })
    }

    pub fn insert<K: AsRef<[u8]>>(&mut self, key: K, ids: u64) -> Result<(), FstError> {
        self.builder.insert(key, ids)?;
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
