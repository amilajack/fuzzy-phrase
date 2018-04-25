enum Word {
    Full {
        word: String,
        id: u64,
        edit_distance: u64,
    },
    Prefix {
        word: String,
        id_range: (u64, u64),
        edit_distance: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_and_prefix_logic() {
        // TODO: write test <24-04-18, boblannon> //
    }
}
