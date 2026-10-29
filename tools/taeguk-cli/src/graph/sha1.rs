//! SHA-1, only to compute git object ids of working tree files (git's default object format).
//! Not used for anything security related.

/// Streaming SHA-1: memory stays one block however large the input.
pub(super) struct Sha1 {
    h: [u32; 5],
    block: Vec<u8>,
    len: u64,
}

impl Sha1 {
    pub(super) fn new() -> Self {
        Sha1 {
            h: [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0],
            block: Vec::with_capacity(64),
            len: 0,
        }
    }

    pub(super) fn update(&mut self, mut data: &[u8]) {
        self.len = self.len.wrapping_add(data.len() as u64);
        while !data.is_empty() {
            let take = (64 - self.block.len()).min(data.len());
            self.block.extend_from_slice(&data[..take]);
            data = &data[take..];
            if self.block.len() == 64 {
                let block = std::mem::take(&mut self.block);
                self.compress(&block);
                self.block = block;
                self.block.clear();
            }
        }
    }

    pub(super) fn hex(mut self) -> String {
        let bits = self.len.wrapping_mul(8);
        let mut tail = vec![0x80u8];
        while (self.block.len() + tail.len()) % 64 != 56 {
            tail.push(0);
        }
        tail.extend_from_slice(&bits.to_be_bytes());
        let len = self.len;
        self.update(&tail);
        self.len = len;
        self.h.iter().map(|x| format!("{x:08x}")).collect()
    }

    fn compress(&mut self, block: &[u8]) {
        let mut w = [0u32; 80];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = self.h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, t);
        }
        for (x, y) in self.h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Sha1;
    fn hex(data: &[u8]) -> String {
        let mut s = Sha1::new();
        // Uneven pieces exercise the block buffer.
        for piece in data.chunks(7) {
            s.update(piece);
        }
        s.hex()
    }
    #[test]
    fn known_vectors() {
        assert_eq!(hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            hex(&[b'a'; 1000]),
            "291e9a6c66994949b57ba5e650361e98fc36b1ba"
        );
        // git hash-object of "hi\n"
        assert_eq!(
            hex(b"blob 3\0hi\n"),
            "45b983be36b73c0788dc9cbcb76cbb80fc7bb057"
        );
    }
}
