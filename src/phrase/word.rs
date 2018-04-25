struct Word {
    word: String,
    id: u64,
    edit_distance: u64,
}

struct WordPrefix {
    word: String,
    id_range: (u64, u64),
    edit_distance: u64,
}
