//! owner: WP-I.13
//!
//! A minimal ZIP reader for the case-package tests: the central directory, stored and
//! deflated entries (RFC 1951, a straightforward canonical-Huffman inflater). The backend
//! has no ZIP dependency and tests add no crate (`rust/Cargo.lock` is the main session's);
//! the archives read here are the ones `asd-export::case_package` writes.
#![allow(dead_code)]

/// One entry: name and uncompressed bytes, in archive order.
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
}

fn u16_at(b: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes([b[at], b[at + 1]]))
}

fn u32_at(b: &[u8], at: usize) -> usize {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) as usize
}

/// Every entry of `zip`, decompressed, in central-directory order. Panics on anything
/// the case package never writes (ZIP64, encryption, other methods).
pub fn entries(zip: &[u8]) -> Vec<Entry> {
    let eocd = (0..=zip.len() - 22)
        .rev()
        .find(|&i| zip[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .expect("end of central directory");
    let count = u16_at(zip, eocd + 10);
    let mut at = u32_at(zip, eocd + 16);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        assert_eq!(zip[at..at + 4], [0x50, 0x4b, 0x01, 0x02], "central header");
        let method = u16_at(zip, at + 10);
        let compressed = u32_at(zip, at + 20);
        let size = u32_at(zip, at + 24);
        let name_len = u16_at(zip, at + 28);
        let extra_len = u16_at(zip, at + 30);
        let comment_len = u16_at(zip, at + 32);
        let local = u32_at(zip, at + 42);
        let name = String::from_utf8(zip[at + 46..at + 46 + name_len].to_vec()).expect("utf-8");
        at += 46 + name_len + extra_len + comment_len;

        assert_eq!(
            zip[local..local + 4],
            [0x50, 0x4b, 0x03, 0x04],
            "local header"
        );
        let start = local + 30 + u16_at(zip, local + 26) + u16_at(zip, local + 28);
        let raw = &zip[start..start + compressed];
        let data = match method {
            0 => raw.to_vec(),
            8 => inflate(raw),
            m => panic!("compression method {m} in {name}"),
        };
        assert_eq!(data.len(), size, "size of {name}");
        out.push(Entry { name, data });
    }
    out
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u32,
    cnt: u32,
}

impl Bits<'_> {
    fn need(&mut self, n: u32) -> u32 {
        while self.cnt < n {
            self.buf |= u32::from(self.data[self.pos]) << self.cnt;
            self.pos += 1;
            self.cnt += 8;
        }
        let v = self.buf & ((1u32 << n) - 1);
        self.buf = if n == 32 { 0 } else { self.buf >> n };
        self.cnt -= n;
        v
    }
}

struct Huffman {
    count: [i32; 16],
    symbol: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Self {
        let mut count = [0i32; 16];
        for &l in lengths {
            count[usize::from(l)] += 1;
        }
        count[0] = 0;
        let mut offs = [0usize; 16];
        for l in 1..15 {
            offs[l + 1] = offs[l] + count[l] as usize;
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[offs[usize::from(l)]] = s as u16;
                offs[usize::from(l)] += 1;
            }
        }
        Self { count, symbol }
    }

    fn decode(&self, bits: &mut Bits<'_>) -> usize {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= bits.need(1) as i32;
            let count = self.count[len];
            if code - count < first {
                return usize::from(self.symbol[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        panic!("invalid Huffman code");
    }
}

const LBASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEXT: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DBASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

fn codes(bits: &mut Bits<'_>, out: &mut Vec<u8>, lit: &Huffman, dist: &Huffman) {
    loop {
        let sym = lit.decode(bits);
        match sym {
            0..=255 => out.push(sym as u8),
            256 => return,
            _ => {
                let i = sym - 257;
                let len = LBASE[i] + bits.need(LEXT[i]) as usize;
                let d = dist.decode(bits);
                let back = DBASE[d] + bits.need(DEXT[d]) as usize;
                let from = out.len() - back;
                for k in 0..len {
                    out.push(out[from + k]);
                }
            }
        }
    }
}

/// RFC 1951 raw deflate.
pub fn inflate(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits {
        data,
        pos: 0,
        buf: 0,
        cnt: 0,
    };
    let mut out = Vec::new();
    loop {
        let last = bits.need(1) == 1;
        match bits.need(2) {
            0 => {
                bits.buf = 0;
                bits.cnt = 0;
                let len = u16_at(data, bits.pos);
                bits.pos += 4;
                out.extend_from_slice(&data[bits.pos..bits.pos + len]);
                bits.pos += len;
            }
            1 => {
                let mut lengths = [0u8; 288];
                for (s, l) in lengths.iter_mut().enumerate() {
                    *l = match s {
                        0..=143 => 8,
                        144..=255 => 9,
                        256..=279 => 7,
                        _ => 8,
                    };
                }
                let lit = Huffman::new(&lengths);
                let dist = Huffman::new(&[5u8; 30]);
                codes(&mut bits, &mut out, &lit, &dist);
            }
            2 => {
                let nlen = bits.need(5) as usize + 257;
                let ndist = bits.need(5) as usize + 1;
                let ncode = bits.need(4) as usize + 4;
                let mut cl = [0u8; 19];
                for &o in ORDER.iter().take(ncode) {
                    cl[o] = bits.need(3) as u8;
                }
                let clh = Huffman::new(&cl);
                let mut lengths = vec![0u8; nlen + ndist];
                let mut i = 0;
                while i < nlen + ndist {
                    let sym = clh.decode(&mut bits);
                    let (value, repeat) = match sym {
                        0..=15 => (sym as u8, 1),
                        16 => (lengths[i - 1], 3 + bits.need(2) as usize),
                        17 => (0, 3 + bits.need(3) as usize),
                        _ => (0, 11 + bits.need(7) as usize),
                    };
                    for _ in 0..repeat {
                        lengths[i] = value;
                        i += 1;
                    }
                }
                let lit = Huffman::new(&lengths[..nlen]);
                let dist = Huffman::new(&lengths[nlen..]);
                codes(&mut bits, &mut out, &lit, &dist);
            }
            t => panic!("invalid deflate block type {t}"),
        }
        if last {
            return out;
        }
    }
}
