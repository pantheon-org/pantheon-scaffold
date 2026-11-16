//! Base64 (standard alphabet, with padding), for the tests that read what a script sent through the GitHub API. Nothing else needs it.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn decode(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for c in text.bytes().filter(|c| *c != b'=') {
        let value = ALPHABET
            .iter()
            .position(|a| *a == c)
            .unwrap_or_else(|| panic!("not base64: {}", char::from(c)));
        buffer = (buffer << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_round_trips_every_padding_length() {
        for text in ["", "a", "ab", "abc", "abcd", "version = \"0.3.1\"\n"] {
            assert_eq!(decode(&encode(text.as_bytes())), text.as_bytes());
        }
        assert_eq!(encode(b"Man"), "TWFu");
        assert_eq!(encode(b"Ma"), "TWE=");
        assert_eq!(encode(b"M"), "TQ==");
    }
}
