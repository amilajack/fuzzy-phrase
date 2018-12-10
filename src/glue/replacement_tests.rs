extern crate tempfile;
extern crate lazy_static;

use super::*;

lazy_static! {
    static ref DIRECTORY: tempfile::TempDir = tempfile::tempdir().unwrap();
    static ref REPLACEMENTS: Vec<WordReplacement> = vec![
        WordReplacement { from: "street".to_string(), to: "st".to_string() },
        WordReplacement { from: "saint".to_string(), to: "st".to_string() },
        WordReplacement { from: "avenue".to_string(), to: "ave".to_string() },
        WordReplacement { from: "fort".to_string(), to: "ft".to_string() },
        WordReplacement { from: "road".to_string(), to: "rd".to_string() },
    ];
    static ref TEST_SET: FuzzyPhraseSet = {
        let mut builder = FuzzyPhraseSetBuilder::new(&DIRECTORY.path()).unwrap();

        builder.load_word_replacements(REPLACEMENTS.clone());

        builder.insert_str("100 main street").unwrap();
        builder.insert_str("100 main st").unwrap();
        builder.insert_str("100 maine st").unwrap();
        builder.insert_str("100 ft wayne rd").unwrap();
        builder.insert_str("100 fortenberry ave").unwrap();

        builder.finish().unwrap();
        FuzzyPhraseSet::from_path(&DIRECTORY.path()).unwrap()
    };
}

fn id_of(word: &str) -> u32 {
    TEST_SET.prefix_set.lookup(word).id().unwrap().value() as u32
}

#[test]
fn load_word_replacements() -> () {
    lazy_static::initialize(&TEST_SET);
    let word_replacement_reader = BufReader::new(fs::File::open(&DIRECTORY.path().join(Path::new("metadata.json"))).unwrap());

    let test_word_replacement: FuzzyPhraseSetMetadata = serde_json::from_reader(word_replacement_reader).unwrap();

    assert_eq!(test_word_replacement.word_replacements, *REPLACEMENTS);
}

#[test]
fn get_nonterminal_word_possibilities() -> () {
    // regular lookup
    assert_eq!(
        TEST_SET.get_nonterminal_word_possibilities("wayne", 1).unwrap().unwrap(),
        vec![QueryWord::new_full(id_of("wayne"), 0)]
    );

    // typo lookup
    assert_eq!(
        TEST_SET.get_nonterminal_word_possibilities("main", 1).unwrap().unwrap(),
        vec![
            QueryWord::new_full(id_of("main"), 0),
            QueryWord::new_full(id_of("maine"), 1),
        ]
    );

    // replacements:
    // standard replacement -- matches replaced word, doesn't match typos of replaced word
    assert_eq!(
        TEST_SET.get_nonterminal_word_possibilities("street", 1).unwrap().unwrap(),
        vec![QueryWord::new_full(id_of("st"), 0)]
    );
    // word is a replacement *target*, so no replacement occurs, and we match both the word
    // and a typo
    assert_eq!(
        TEST_SET.get_nonterminal_word_possibilities("st", 1).unwrap().unwrap(),
        vec![
            QueryWord::new_full(id_of("st"), 0),
            QueryWord::new_full(id_of("ft"), 1),
        ]
    );
    // match a typo of the looked-up word and then replace
    assert_eq!(
        TEST_SET.get_nonterminal_word_possibilities("stret", 1).unwrap().unwrap(),
        vec![QueryWord::new_full(id_of("st"), 1)]
    );
    // match nothing on prefix matches
    assert_eq!(
        TEST_SET.get_nonterminal_word_possibilities("s", 1).unwrap(),
        None
    );
    // spelling-correct str to st
    assert_eq!(
        TEST_SET.get_nonterminal_word_possibilities("str", 1).unwrap().unwrap(),
        vec![QueryWord::new_full(id_of("st"), 1)]
    );
    // match nothing on prefix match of replacement source that can't be a typo
    assert_eq!(
        TEST_SET.get_nonterminal_word_possibilities("stre", 1).unwrap(),
        None
    );
}

#[test]
fn get_terminal_word_possibilities() -> () {
    // regular lookup -- emitted as a full word because it has no continuations
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("wayne", 1).unwrap().unwrap(),
        vec![QueryWord::new_full(id_of("wayne"), 0)]
    );

    // typo lookup -- don't include typo if it would be covered by the prefix anyway
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("main", 1).unwrap().unwrap(),
        vec![
            QueryWord::new_prefix((id_of("main"), id_of("maine")))
        ]
    );

    // replacements:
    // standard replacement -- matches replaced word, doesn't match typos of replaced word
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("street", 1).unwrap().unwrap(),
        vec![QueryWord::new_full(id_of("st"), 0)]
    );
    // match the prefixd, and also a typo
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("st", 1).unwrap().unwrap(),
        vec![
            QueryWord::new_prefix((id_of("st"), id_of("street"))),
            QueryWord::new_full(id_of("ft"), 1),
        ]
    );
    // match a typo of the looked-up word and then replace
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("stret", 1).unwrap().unwrap(),
        vec![QueryWord::new_full(id_of("st"), 1)]
    );
    // match all the s words on prefix match
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("s", 1).unwrap().unwrap(),
        vec![
            QueryWord::new_prefix((id_of("saint"), id_of("street")))
        ]
    );
    // the only possibility here is for a completion, so just emit that, as a full word
    // (we could alternatively interpret str as a typo of st instead of a prefix, but we don't
    // because the prefix version is lower-distance so it takes precedence)
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("str", 1).unwrap().unwrap(),
        vec![QueryWord::new_full(id_of("st"), 0)]
    );

    // ft/fort/forenberry:
    // we just need a prefix here since f covers everything
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("f", 1).unwrap().unwrap(),
        vec![
            QueryWord::new_prefix((id_of("fort"), id_of("ft"))),
        ]
    );
    // here we need both the prefix and the full word, because one possible termination gets
    // replaced and the other doesn't (note that we don't include a fuzzy possibility)
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("fo", 1).unwrap().unwrap(),
        vec![
            QueryWord::new_prefix((id_of("fort"), id_of("fortenberry"))),
            QueryWord::new_full(id_of("ft"), 0)
        ]
    );
    // same as above even though this is now a full replaceable word
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("fort", 1).unwrap().unwrap(),
        vec![
            QueryWord::new_prefix((id_of("fort"), id_of("fortenberry"))),
            QueryWord::new_full(id_of("ft"), 0)
        ]
    );
    // now we only emit the prefix option, with just one element
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("forten", 1).unwrap().unwrap(),
        vec![
            QueryWord::new_prefix((id_of("fortenberry"), id_of("fortenberry")))
        ]
    );
    // and finally, emit a full word once we have the whole word
    assert_eq!(
        TEST_SET.get_terminal_word_possibilities("fortenberry", 1).unwrap().unwrap(),
        vec![
            QueryWord::new_full(id_of("fortenberry"), 0)
        ]
    );
}

#[test]
fn contains() {
    assert_eq!(
        TEST_SET.contains_str("100 ft wayne rd", EndingType::NonPrefix).unwrap(),
        true
    );

    assert_eq!(
        TEST_SET.contains_str("100 fort wayne road", EndingType::NonPrefix).unwrap(),
        true
    );
}

#[test]
fn contains_prefix() {
    for variant in vec![
        "100 fort wayne road",
        "100 ft wayne road",
        "100 fort wayne roa",
        "100 ft wayne roa",
        "100 fort wayne rd",
        "100 ft wayne rd",
        "100 fort wayne r",
        "100 ft wayne r",
        "100 for",
        "100 f"
    ] {
        assert_eq!(
            TEST_SET.contains_str(variant, EndingType::AnyPrefix).unwrap(),
            true
        );
    }
    assert_eq!(
        TEST_SET.contains_str("100 q", EndingType::AnyPrefix).unwrap(),
        false
    );
}

#[test]
fn fuzzy_match() {
    // match nothing -- the only way to get from s to st without prefixes is fuzzy matching,
    // and we don't fuzzy-match one-char words
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main s", 1, 1, EndingType::NonPrefix).unwrap(),
        vec![]
    );

    // exact-match to 100 main st at edit distance 0
    // also match 100 maine st since we're in budget if we don't have any other typos
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main st", 1, 1, EndingType::NonPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "main".to_string(), "st".to_string()], ending_type: EndingType::NonPrefix },
            FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "maine".to_string(), "st".to_string()], ending_type: EndingType::NonPrefix }
        ]
    );

    // match to "100 main st" by fuzzy-matching, at distance 1
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main str", 1, 1, EndingType::NonPrefix).unwrap(),
        vec![FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "main".to_string(), "st".to_string()], ending_type: EndingType::NonPrefix }]
    );

    // don't match anything if fuzzy search is disabled
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main str", 0, 0, EndingType::NonPrefix).unwrap(),
        vec![]
    );

    // match to nothing -- not doing prefix matching and too far to fuzzy match
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main stre", 1, 1, EndingType::NonPrefix).unwrap(),
        vec![]
    );

    // match to "100 main street" by fuzzy-matching and then token-replace to "100 main st" at distance 1
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main stree", 1, 1, EndingType::NonPrefix).unwrap(),
        vec![FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "main".to_string(), "st".to_string()], ending_type: EndingType::NonPrefix }]
    );

    // exact-match to 100 main street and then replace, so match at edit distance 0
    // also match 100 maine st since we're in budget if we don't have any other typos
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main street", 1, 1, EndingType::NonPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "main".to_string(), "st".to_string()], ending_type: EndingType::NonPrefix },
            FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "maine".to_string(), "st".to_string()], ending_type: EndingType::NonPrefix }
        ]
    );

    // make sure token replacements at not-the-end work as well
    for variant in vec!["100 fort wayne road", "100 ft wayne road", "100 fort wayne rd", "100 ft wayne rd"] {
        assert_eq!(
            TEST_SET.fuzzy_match_str(variant, 1, 1, EndingType::NonPrefix).unwrap(),
            vec![
                FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "ft".to_string(), "wayne".to_string(), "rd".to_string()], ending_type: EndingType::NonPrefix }
            ]
        )
    }
}

#[test]
fn fuzzy_match_prefix() {
    // match "100 main s" by prefix at edit distance 0, and since the s doesn't take up any budget,
    // also match "100 maine s"
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main s", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "main".to_string(), "s".to_string()], ending_type: EndingType::AnyPrefix },
            FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "maine".to_string(), "s".to_string()], ending_type: EndingType::AnyPrefix }
        ]
    );

    // exact-match to 100 main st at edit distance 0
    // also match 100 maine st since we're in budget if we don't have any other typos
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main st", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "main".to_string(), "st".to_string()], ending_type: EndingType::AnyPrefix },
            FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "maine".to_string(), "st".to_string()], ending_type: EndingType::AnyPrefix }
        ]
    );

    // match to "100 main st" by detecting replaceable prefix, at edit distance 0 (and maine)
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main str", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "main".to_string(), "st".to_string()], ending_type: EndingType::WordBoundaryPrefix },
            FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "maine".to_string(), "st".to_string()], ending_type: EndingType::WordBoundaryPrefix }
        ]
    );

    // omit maine but still match main if fuzzy search is disabled
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main str", 0, 0, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "main".to_string(), "st".to_string()], ending_type: EndingType::WordBoundaryPrefix },
        ]
    );

    // match to "100 main st" by detecting replaceable prefix, at edit distance 0 (and maine)
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main stre", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "main".to_string(), "st".to_string()], ending_type: EndingType::WordBoundaryPrefix },
            FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "maine".to_string(), "st".to_string()], ending_type: EndingType::WordBoundaryPrefix }
        ]
    );

    // match to "100 main st" by detecting replaceable prefix, at edit distance 0 (and maine)
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main stree", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "main".to_string(), "st".to_string()], ending_type: EndingType::WordBoundaryPrefix },
            FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "maine".to_string(), "st".to_string()], ending_type: EndingType::WordBoundaryPrefix }
        ]
    );

    // exact-match to 100 main street and then replace, so match at edit distance 0
    // also match 100 maine st since we're in budget if we don't have any other typos
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 main street", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "main".to_string(), "st".to_string()], ending_type: EndingType::WordBoundaryPrefix },
            FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "maine".to_string(), "st".to_string()], ending_type: EndingType::WordBoundaryPrefix }
        ]
    );

    // this matches one thing that will be replaced and one thing that won't, but they both share
    // a prefix so just one thing comes out
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 f", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "f".to_string()], ending_type: EndingType::AnyPrefix }
        ]
    );

    // here they diverge: "fo" matches both "fort" (which will get replaced with "ft") and "fortenberry"
    // (which won't be replaced), so we need to emit both the regular prefix and the replacement
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 fo", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "fo".to_string()], ending_type: EndingType::AnyPrefix },
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "ft".to_string()], ending_type: EndingType::WordBoundaryPrefix }
        ]
    );

    // this one, interestingly, matches "100 ft" in two different ways: by fuzzy-matching to "100 ft",
    // and by fuzzy-matching to "100 fort" and then replacing. Only one happened here but it doesn't
    // matter which -- we should see only one response, "100 ft" at distance 1 (and not fortenberry
    // at all)
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 frt", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "ft".to_string()], ending_type: EndingType::WordBoundaryPrefix }
        ]
    );

    // including the whole replaceable word behaves ~identically to including only its prefix
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 fort", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "fort".to_string()], ending_type: EndingType::AnyPrefix },
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "ft".to_string()], ending_type: EndingType::WordBoundaryPrefix }
        ]
    );

    // now we prefer fortenberry but also consider fort -> ft by fuzzy-match
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 forte", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "forte".to_string()], ending_type: EndingType::AnyPrefix },
            FuzzyMatchResult { edit_distance: 1, phrase: vec!["100".to_string(), "ft".to_string()], ending_type: EndingType::WordBoundaryPrefix }
        ]
    );

    // now all that's left is fortenberry -- fort is too far away
    assert_eq!(
        TEST_SET.fuzzy_match_str("100 forten", 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "forten".to_string()], ending_type: EndingType::AnyPrefix },
        ]
    );

    // make sure token replacements at not-the-end work as well, and interact with autocomplete, etc., well
    for variant in vec![
        "100 fort wayne road",
        "100 ft wayne road",
        "100 fort wayne roa",
        "100 ft wayne roa",
        "100 fort wayne rd",
        "100 ft wayne rd"
    ] {
        assert_eq!(
            TEST_SET.fuzzy_match_str(variant, 1, 1, EndingType::AnyPrefix).unwrap(),
            vec![
                FuzzyMatchResult { edit_distance: 0, phrase: vec!["100".to_string(), "ft".to_string(), "wayne".to_string(), "rd".to_string()], ending_type: EndingType::WordBoundaryPrefix }
            ]
        )
    }
}

#[test]
fn fuzzy_match_windows() -> () {
    // make sure token replacements at not-the-end work as well, and interact with autocomplete, etc., well
    for variant in vec![
        "100 fort wayne road",
        "100 ft wayne road",
        "100 fort wayne roa",
        "100 ft wayne roa",
        "100 fort wayne rd",
        "100 ft wayne rd"
    ] {
        assert_eq!(
            TEST_SET.fuzzy_match_windows(&variant.split(' ').collect::<Vec<_>>(), 1, 1, EndingType::AnyPrefix).unwrap(),
            vec![
                FuzzyWindowResult {
                    edit_distance: 0,
                    phrase: vec!["100".to_string(), "ft".to_string(), "wayne".to_string(), "rd".to_string()],
                    start_position: 0,
                    ending_type: EndingType::WordBoundaryPrefix
                }
            ]
        )
    }

    for variant in vec![
        "100 fort wayne road washington dc",
        "100 ft wayne road washington dc",
        "100 fort wayne rd washington dc",
        "100 ft wayne rd washington dc"
    ] {
        assert_eq!(
            TEST_SET.fuzzy_match_windows(&variant.split(' ').collect::<Vec<_>>(), 1, 1, EndingType::AnyPrefix).unwrap(),
            vec![
                FuzzyWindowResult {
                    edit_distance: 0,
                    phrase: vec!["100".to_string(), "ft".to_string(), "wayne".to_string(), "rd".to_string()],
                    start_position: 0,
                    ending_type: EndingType::NonPrefix
                }
            ]
        )
    }

    for variant in vec![
        "washington dc 100 fort wayne road",
        "washington dc 100 ft wayne road",
        "washington dc 100 fort wayne rd",
        "washington dc 100 ft wayne rd"
    ] {
        assert_eq!(
            TEST_SET.fuzzy_match_windows(&variant.split(' ').collect::<Vec<_>>(), 1, 1, EndingType::AnyPrefix).unwrap(),
            vec![
                FuzzyWindowResult {
                    edit_distance: 0,
                    phrase: vec!["100".to_string(), "ft".to_string(), "wayne".to_string(), "rd".to_string()],
                    start_position: 2,
                    ending_type: EndingType::WordBoundaryPrefix
                }
            ]
        )
    }

    // make sure emitting multiple things works with replacement
    assert_eq!(
        TEST_SET.fuzzy_match_windows(&["washington", "dc", "100", "fo"], 1, 1, EndingType::AnyPrefix).unwrap(),
        vec![
            FuzzyWindowResult {
                edit_distance: 0,
                phrase: vec!["100".to_string(), "fo".to_string()],
                start_position: 2,
                ending_type: EndingType::AnyPrefix
            },
            FuzzyWindowResult {
                edit_distance: 0,
                phrase: vec!["100".to_string(), "ft".to_string()],
                start_position: 2,
                ending_type: EndingType::WordBoundaryPrefix
            }
        ]
    );
}

#[test]
fn multi_search_fuzzy_match_equivalence() -> () {
    assert_eq!(
        TEST_SET.fuzzy_match_multi(&[
            (vec!["100", "main", "s"], EndingType::NonPrefix),
            (vec!["100", "main", "st"], EndingType::NonPrefix),
            (vec!["100", "main", "str"], EndingType::NonPrefix),
            (vec!["100", "main", "stre"], EndingType::NonPrefix),
            (vec!["100", "main", "stree"], EndingType::NonPrefix),
            (vec!["100", "main", "street"], EndingType::NonPrefix),
            (vec!["100", "fort", "wayne", "road"], EndingType::NonPrefix),
            (vec!["100", "ft", "wayne", "road"], EndingType::NonPrefix),
            (vec!["100", "fort", "wayne", "rd"], EndingType::NonPrefix),
            (vec!["100", "ft", "wayne", "rd"], EndingType::NonPrefix),
            (vec!["100", "main", "s"], EndingType::AnyPrefix),
            (vec!["100", "main", "st"], EndingType::AnyPrefix),
            (vec!["100", "main", "str"], EndingType::AnyPrefix),
            (vec!["100", "main", "stre"], EndingType::AnyPrefix),
            (vec!["100", "main", "stree"], EndingType::AnyPrefix),
            (vec!["100", "main", "street"], EndingType::AnyPrefix),
            (vec!["100", "fort", "wayne", "road"], EndingType::AnyPrefix),
            (vec!["100", "ft", "wayne", "road"], EndingType::AnyPrefix),
            (vec!["100", "fort", "wayne", "rd"], EndingType::AnyPrefix),
            (vec!["100", "ft", "wayne", "rd"], EndingType::AnyPrefix),
        ], 1, 1).unwrap(),
        vec![
            TEST_SET.fuzzy_match(&["100", "main", "s"], 1, 1, EndingType::NonPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "main", "st"], 1, 1, EndingType::NonPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "main", "str"], 1, 1, EndingType::NonPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "main", "stre"], 1, 1, EndingType::NonPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "main", "stree"], 1, 1, EndingType::NonPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "main", "street"], 1, 1, EndingType::NonPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "fort", "wayne", "road"], 1, 1, EndingType::NonPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "ft", "wayne", "road"], 1, 1, EndingType::NonPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "fort", "wayne", "rd"], 1, 1, EndingType::NonPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "ft", "wayne", "rd"], 1, 1, EndingType::NonPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "main", "s"], 1, 1, EndingType::AnyPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "main", "st"], 1, 1, EndingType::AnyPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "main", "str"], 1, 1, EndingType::AnyPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "main", "stre"], 1, 1, EndingType::AnyPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "main", "stree"], 1, 1, EndingType::AnyPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "main", "street"], 1, 1, EndingType::AnyPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "fort", "wayne", "road"], 1, 1, EndingType::AnyPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "ft", "wayne", "road"], 1, 1, EndingType::AnyPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "fort", "wayne", "rd"], 1, 1, EndingType::AnyPrefix).unwrap(),
            TEST_SET.fuzzy_match(&["100", "ft", "wayne", "rd"], 1, 1, EndingType::AnyPrefix).unwrap(),
        ]
    );
}