use std::io::Cursor;
use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};
use std::fmt;
use std::error;

pub fn chop_int(num: u32) -> Vec<u8> {
    let mut wtr = vec![];
    wtr.write_u32::<BigEndian>(num).unwrap();
    wtr
}

pub fn three_byte_encode(num: u32) -> Vec<u8> {
    debug_assert!(num < 16_777_216);
    let chopped: Vec<u8> = chop_int(num);
    let three_bytes: Vec<u8> = chopped[1..4].to_vec();
    three_bytes
}

pub fn three_byte_decode(three_bytes: &[u8]) -> u32 {
    let mut padded_byte_vec: Vec<u8> = vec![0u8; 1];
    padded_byte_vec.extend_from_slice(three_bytes);
    let mut reader = Cursor::new(padded_byte_vec);
    reader.read_u32::<BigEndian>().unwrap()
}

pub fn word_ids_to_key(phrase: &[u32]) -> Vec<u8> {
    phrase.into_iter()
          .flat_map(|word| three_byte_encode(*word))
          .collect()
}

pub fn key_to_word_ids(key: &[u8]) -> Vec<u32> {
    let mut phrase: Vec<u32> = vec![];
    let mut i = 0;
    while i < (key.len() - 2) {
        let word = &key[i..i+3];
        phrase.push(three_byte_decode(&word));
        i += 3;
    }
    phrase
}

#[derive(Debug, Clone)]
pub struct PhraseSetError {
    details: String
}

impl PhraseSetError {
    pub fn new(msg: &str) -> PhraseSetError {
        PhraseSetError{details: msg.to_string()}
    }
}

impl fmt::Display for PhraseSetError {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}
impl error::Error for PhraseSetError {
    fn description(&self) -> &str {
        &self.details
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chop_smallest_int_to_bytes() {
        let n: u32 = u32::min_value();
        let chopped: Vec<u8> = chop_int(n);
        assert_eq!(
            vec![0u8, 0u8, 0u8, 0u8],
            chopped
        );

    }

    #[test]
    fn chop_largest_int_to_bytes() {
        let n: u32 = u32::max_value();
        let chopped: Vec<u8> = chop_int(n);
        assert_eq!(
            vec![255u8, 255u8, 255u8, 255u8],
            chopped
        );
    }

    #[test]
    fn chop_big_int_to_bytes() {
        // first value larger than u16::max_value(), aka 2**16
        let n: u32 = 65_537;
        let chopped: Vec<u8> = chop_int(n);
        assert_eq!(
            vec![0u8, 1u8, 0u8, 1u8],
            chopped
        );
    }

    #[test]
    fn medium_integer_to_three_bytes() {
        // the number we're using is arbitrary.
        let n: u32 = 61_528;
        let three_bytes: Vec<u8> = three_byte_encode(n);
        assert_eq!(
            vec![ 0u8, 240u8, 88u8],
            three_bytes
        );
    }

    #[test]
    fn large_integer_to_three_bytes() {
        // the number we're using is arbitrary. happens to be the number of distinct words in
        // us-address, so gives us an idea of the cardinality we're dealing with.
        let n: u32 = 561_528;
        let three_bytes: Vec<u8> = three_byte_encode(n);
        assert_eq!(
            vec![ 8u8, 145u8, 120u8],
            three_bytes
        );
    }

    #[test]
    #[should_panic]
    fn integer_is_to_large() {
        // we should panic if we try to encode something larger than (2^24 - 1)
        let n: u32 = 16_777_216;
        three_byte_encode(n);
    }

    #[test]
    fn three_bytes_to_large_integer() {
        let three_bytes: Vec<u8> = vec![ 8u8, 145u8, 120u8];
        let n: u32 = three_byte_decode(&three_bytes);
        assert_eq!(
            561_528u32,
            n
        );
    }

    #[test]
    fn convert_word_ids_to_key() {
        let word_ids = [61_528_u32, 561_528u32, 1u32];
        let key = word_ids_to_key(&word_ids);
        assert_eq!(
            vec![
                0u8, 240u8, 88u8,    // 61_528
                8u8, 145u8, 120u8,   // 561_528
                0u8, 0u8,   1u8      // 1
            ],
            key
        );
    }

    #[test]
    fn convert_key_to_word_ids() {
        let key =vec![
            0u8, 240u8, 88u8,    // 61_528
            8u8, 145u8, 120u8,   // 561_528
            0u8, 0u8,   1u8      // 1
        ];
        let word_ids = key_to_word_ids(&key);
        assert_eq!(
            vec![61_528_u32, 561_528u32, 1u32],
            word_ids
        );
    }

}

