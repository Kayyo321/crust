/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct bitset {
    words: Vec<u64>,
}

impl bitset {
    pub fn new(n: usize) -> bitset {
        bitset { words: vec![0; n.div_ceil(64)] }
    }

    pub fn full(n: usize) -> bitset {
        let mut b = bitset::new(n);
        for i in 0..n {
            b.set(i);
        }
        b
    }

    pub fn get(&self, i: usize) -> bool {
        self.words[i / 64] >> (i % 64) & 1 == 1
    }

    pub fn set(&mut self, i: usize) {
        self.words[i / 64] |= 1 << (i % 64);
    }

    pub fn clear(&mut self, i: usize) {
        self.words[i / 64] &= !(1 << (i % 64));
    }

    /// Unions `other` into `self`. Returns whether `self` changed.
    pub fn union(&mut self, other: &bitset) -> bool {
        let mut changed = false;
        for (a, b) in self.words.iter_mut().zip(&other.words) {
            let n = *a | *b;
            changed |= n != *a;
            *a = n;
        }
        changed
    }
}
