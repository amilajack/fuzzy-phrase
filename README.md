# fuzzy-phrase

![kelsey-grammer-x-men](https://user-images.githubusercontent.com/78930/38650072-bf21d582-3dae-11e8-9f9e-bccb6a44b7ba.jpg)

_Mascot: Fuzzy Frasier_

fuzzy-phrase is a fuzzy (and exact) phrase matching engine written in Rust and built to power textual indexing and lookup for [carmen](https://github.com/mapbox/carmen). This repository contains a pure-Rust library; it has a companion repository, [node-fuzzy-phrase](https://github.com/mapbox/node-fuzzy-phrase), that exposes its functionality to Node/Javascript for carmen’s use.

# Getting started

fuzzy-phrase is a standard Rust crate. It isn’t currently published on crates.io, so to get started you can add an entry to your `Cargo.toml` as follows:

    fuzzy-phrase = { git = "https://github.com/mapbox/fuzzy-phrase", rev = "master" }

To use it, you’ll build a structure by instantiating a `glue::FuzzyPhraseSetBuilder` and add some phrases to it. You can then load the built structure and query it for approximate matches:

    let mut builder = FuzzyPhraseSetBuilder::new(&DIR.path()).unwrap();
    builder.insert_str("100 main street").unwrap();
    builder.insert_str("200 main street").unwrap();
    builder.insert_str("100 main ave").unwrap();
    builder.insert_str("300 mlk blvd").unwrap();
    builder.finish().unwrap();
    
    let set = FuzzyPhraseSet::from_path(&DIR.path()).unwrap();
    println!("{:?}", SET.fuzzy_match(&["100", "man", "street"], 1, 1).unwrap());

fuzzy-phrase uses standard Rust tests, so you can run the test suite using

    cargo test

and run benchmarks (implemented using `criterion`) using

    cargo bench

# How it works

fuzzy-phrase’s function is to index and allow the lookup of phrases (for example, the names of geographical features, such as “100 Main St” or “New Brunswick”). Each fuzzy-phrase instance has an initial one-time creation and indexing step, and is read-only thereafter. At indexing time, the library constructs a static lexicon of all the words any of its phrases contain, and stores the words separately from the phrases they form, each of which is stored as a sequence of word IDs. Words themselves are stored in two different representations, one to allow fuzzy matching (i.e., spelling correction), and one to allow for prefix matching, to support autocomplete.

There are thus three basic data structures, each of which is implemented in its own module in this crate. All three rely on the `fst` crate to provide underlying data storage. Each module supplies one type for building instances of itself at index time, and one type for read-only querying of them at query time. There is a fourth module, `glue`, which orchestrates the interactions of the first three.

# Word prefix graph

**Module:** `prefix`<br />
**Builder:** `prefix::PrefixSetBuilder`<br />
**Reader:** `prefix::PrefixSet`

This graph contains all the words in the instance’s lexicon, structured so as to allow two kinds of lookups:

- “is this word in the lexicon, and if so, what is its ID?” (`prefix::PrefixSet::get`)
- “is this word the prefix of one or more words in the lexicon, and if so what is the range of IDs representing the words beginning with this prefix?” (`prefix::PrefixSet::get_prefix_range`)

# Fuzzy word graph

**Module:** `fuzzy`<br />
**Builder:** `fuzzy::FuzzyMapBuilder`<br />
**Reader:** `fuzzy::FuzzyMap`

This graph also contains all the words in the instance’s lexicon, this time structured to allow a different kind of query:

- “are any words within edit distance X of this word within the lexicon, and if so, what are their IDs, and what are their edit distances from this word?” (`fuzzy::FuzzyMap::lookup`)

We use the representation proposed in the Symmetric Delete algorithm ([SymSpell](https://github.com/wolfgarbe/SymSpell)) to store words in this graph. In other words, given a word “house,” we will store all words [“house”, “ouse”, “huse”, “hose”, “houe”, “hous”] in the index, each mapped to the ID for “house.” This means our maximum edit distance is fixed at structure construction (indexing) time, and is **currently hard-coded library-wide to 1.** The distance metric we use is [Modified Damerau-Levenshtein distance (also known as Optimal String Alignment distance)](https://en.wikipedia.org/wiki/Damerau%E2%80%93Levenshtein_distance#Optimal_string_alignment_distance), though at an edit distance of 1, MDL and standard Damerau-Levenshtein distance are equivalently expressive.

# Phrase graph

**Module:** `phrase`<br />
**Builder:** `phrase::PhraseSetBuilder`<br />
**Reader:** `phrase::PhraseSet`

This graph contains all the phrases in the index, stored as sequences of word IDs. Because the underlying `fst` representation treats each entry as a byte sequence, we transform each word into a big-endian sequence of three bytes (allowing 2^24 possible words per index), and each phrase as a byte sequence of a multiple-of-three length.

This graph lets us answer several different questions, in order of increasing esotericity:

- “does this graph contain this sequence of word IDs?” (`phrase::PhraseSet::contains`)
- “does this graph contain any phrases that start with this sequence of word IDs (the last of which might be a range of word IDs rather than a single ID)?” (`phrase::PhraseSet::contains_prefix`)
- “given a list of word positions where for each position, multiple intended word IDs have been identified at different edit distances, which combinations consisting of one candidate word ID for each slot exist in this graph, constrained to a given maximum total edit distance? if so, what are they and what are their total respective edit distances?” (`phrase::PhraseSet::match_combinations`)
- same as above, but for phrase prefixes rather than whole phrases (`phrase::PhraseSet::match_combinations_as_prefixes`)
- “given a similar list of word positions representing a query whose ideal match spans more than one index, are there any substrings of any combinations of words that exist in this graph? if so, what are they, where do they start and stop, and what are their total respective edit distances?” (`phrase::PhraseSet::match_combinations_as_windows` with parameter `ends_in_prefix` set to false)
- same as above, but allowing for the possibility that a substring including the terminal word might be a phrase prefix rather than a whole phrase (`phrase::PhraseSet::match_combinations_as_windows` with parameter `ends_in_prefix` set to true)

# Glue

The `glue` module does not supply any new data structures of its own, but instead orchestrates the querying of the three main structures and supplies an outward-facing set of structures for building and querying them in concert.

**Module:** `glue`<br />
**Builder:** `glue::FuzzyPhraseSetBuilder`<br />
**Reader:** `glue::FuzzyPhraseSet`

This module allows us to answer the high-level questions that combine querying of the three underlying graphs:

- “does this structure contain this sequence of words?” (`glue::FuzzyPhraseSet::contains`, achieved by combining `prefix::PrefixSet::get` with `phrase::PhraseSet::contains`)
- “does this structure contain anything starting with this sequence of words, the last of which might be incomplete?” (`glue::FuzzyPhraseSet::contains_prefix`, combining `prefix::PrefixSet::get_prefix_range` with `glue::FuzzyPhraseSet::contains_prefix`)
- “does this structure contain anything within total edit distance X of this sequence of words?” (`glue::FuzzyPhraseSet::fuzzy_match`, combining `fuzzy::FuzzyMap::lookup` and `phrase::PhraseSet::match_combinations`)
- same as above, but as a prefix match (`glue::FuzzyPhraseSet::fuzzy_match_prefix`, combining `fuzzy::FuzzyMap::lookup`,  `prefix::PrefixSet::get_prefix_range`, and `phrase::PhraseSet::match_combinations_as_prefixes`)
- “does this structure contain any phrases within edit distance X of any subsequence of words within this sequence, either with or without prefix matching?” (`glue::FuzzyPhraseSet::fuzzy_match_windows`, combining `fuzzy::FuzzyMap::lookup`,  `prefix::PrefixSet::get_prefix_range`, and `phrase::PhraseSet::match_combinations_as_windows`)
- “does this structure contain any phrases within edit distance X of any of the following list of sequences of words, some of which might allow for prefix matching?” (`glue::FuzzyPhraseSet::fuzzy_match_multi`, combining `fuzzy::FuzzyMap::lookup`,  `prefix::PrefixSet::get_prefix_range`, and `phrase::PhraseSet::match_combinations_as_windows`) — note that the results of this function are identical to the results you’d get from multiple calls to `fuzzy_match` or `fuzzy_match_prefix`, but can be carried out more efficiently if multiple phrases within the last share words, as spelling correction operations can be shared

# Other implementation details

At present we don’t attempt to spelling-correct any word containing a digit, or any word containing a character that isn’t Latin, Greek, or Cyrillic. We do exact lookups of these words instead. Similarly, we don’t attempt to spelling-correct single-letter words.

# An example lookup

To make the above more concrete, here’s the process for how we’d perform a single fuzzy prefix lookup (`glue::FuzzyPhraseSet::fuzzy_match_prefix`) of one phrase. The rough process generalizes to the more complex variants as well.

We’ll consider how a case where our user wants to look for “100 west main street” but has made a mistake and also isn’t done typing yet, and has so far typed “100 west man stre”. We’ll further assume that our maximum edit distance over the entire phrase that we’re willing to tolerate is 1.

## Word by word

First, we’ll want to figure out all the alternative words that might be intended for each token, and what edit distance, if any, each one is from our target word. We’ll try applying spelling correction to tokens made up entirely of letters (punting if they include numbers or non-alphabetic characters like CJK), and we’ll apply prefix completion to the final token in the phrase, which is the only one the user might not be done typing yet. If the final token is made of letters, we’ll apply both, but not simultaneously: we’ll consider the possibility that the word is incomplete, or that the word is complete but misspelled, but not both at once (which would need a single combined structure fulfilling both functions)

For case where we’re applying neither spelling correction nor prefix completion, all we’re doing is a `get` operation in our `PrefixSet`, which will produce the ID for that word if it exists in the graph, or failure of not. For “100” as an example which contains numbers and so can’t be spell-checked, we’ll determine that it *is* in the graph, and its ID is, say, 47. As it’s an exact match, we’ll assign it an edit distance of 0.

For the case where we’ll try to apply spelling correction, we’ll use the `lookup` function of our `FuzzyMap` instead, which will apply the SymSpell algorithm. This might produce multiple possible matches at different edit distances. For example, considering our lookup of “west”, we might end up with the possibilities (“west”, ID 205, distance 0) and (“best”, ID 160, distance 1).

For the case where we’re applying prefix completion, we’re looking at a string that is the beginning of potentially many words. Rather than trying to enumerate them all, all we want to do is figure out (a) if that prefix is in the graph, (b) what the *range of IDs* all the words with that prefix fall in, taking advantage of the fact that we’ve assigned our word IDs in lexicographical order, and all words sharing a given prefix will be grouped together lexicographically and so will share a single range contiguous range of IDs. For “stre”, we’ll find that that prefix *is* in our `PrefixSet`, and represents the start of words from 195 to 197, hereafter `[195,198)`  in inclusive/exclusive notation (perhaps “straight”, “stream”, and “street”, but we’re not actually trying to figure that out right now). We don’t apply distance penalties to autocomplete matches, to the output of this operation will be (“stre”, ID-range [195,198), distance 0). We’ll also separately try a spelling correction operation on this word as above, and produce (“store”, ID 192, distance 1).

This gives us the following variant set:

| *token*    | **100**        | **west**                          | **man**                          | **stre**                                 |
| ---------- | -------------- | --------------------------------- | -------------------------------- | ---------------------------------------- |
| *variants* | (“100”, 47, 0) | (“west”, 205, 0)<br />(“best”, 160, 1) | (“man”, 180, 0)<br />(“main”, 178, 1) | (“stre”, [195,198), 0)<br />(“store”, 192, 1) |
| *strategy* | exact only     | spellcheck only                   | spellcheck only                  | prefix + spellcheck                      |

Note: for a regular fuzzy match, if any of the tokens returned no results at this point, we’re done; we don’t need to consider the phrase graph at all and can return no results now. For other kinds of lookups such as windowed lookups, we may still be able to proceed. Regardless, since in our example we did get at least one result for each token, we can continue.

## Phrases

Next, we’ll consider which possible combinations of candidate words in each slot might form phrases contained in our `PhraseSet`. In practice, we don’t actually enumerate all the possible phrases, but rather, perform a depth-first search of our phrase graph, pruning branches as we go either in response to match failures or to exceeding our maximum edit distance threshold; see `PhraseSet::match_combinations_as_prefixes` for the implementation.

For simplicity, though, let’s say we did enumerate them all in advance. Assuming we keep our whole-phrase maximum edit distance of 1 in mind, and eliminate any combinations with a total distance exceeding that limit, we might end up with a list like this:

1. [(“100”, 47, 0), (“west”, 205, 0), (“man”, 180, 0), (“stre”, [195,198), 0)]
2. [(“100”, 47, 0), (“best”, 160, 1), (“man”, 180, 0), (“stre”, [195,198), 0)]
3. [(“100”, 47, 0), (“west”, 205, 0), (“main”, 178, 1), (“stre”, [195,198), 0)]
4. [(“100”, 47, 0), (“west”, 205, 0), (“man”, 180, 0), (“store”, 192, 1)]

Because this is a prefix lookup, the question we want to answer is either:

- for entries that end in a range (i.e., #1-#3), does our `PhraseSet` contain any phrases that begin with all the complete words, followed by a word in our terminal range?
- for entries without a range, does our `PhraseSet` contain any phrases that start with this entry? (#4)

We discover through this process that only candidate #3 is in the graph, so we return a list with one result: [“100”, “west”, “main”, “stre”], with an edit distance of 1.

