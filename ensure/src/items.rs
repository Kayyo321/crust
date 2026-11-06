/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Checks on things that hold other things: text, lists, arrays, sets, and maps.
use crate::report::show;
use crate::subjects::subject;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Debug;

/// Something with a length: text (in bytes), a list, a set, or a map.
pub trait measured {
    fn measure(&self) -> usize;
}

/// Something that can contain a `needle`: text holds `&str`, `char`, and `String`; a list holds
/// its items.
pub trait holds<needle> {
    fn holds_needle(&self, needle: &needle) -> bool;
}

impl<M: measured + ?Sized> measured for &M {
    fn measure(&self) -> usize {
        (**self).measure()
    }
}

impl measured for str {
    fn measure(&self) -> usize {
        self.len()
    }
}

impl measured for String {
    fn measure(&self) -> usize {
        self.len()
    }
}

impl<T> measured for [T] {
    fn measure(&self) -> usize {
        self.len()
    }
}

impl<T> measured for Vec<T> {
    fn measure(&self) -> usize {
        self.len()
    }
}

impl<T, const N: usize> measured for [T; N] {
    fn measure(&self) -> usize {
        N
    }
}

impl<T> measured for HashSet<T> {
    fn measure(&self) -> usize {
        self.len()
    }
}

impl<T> measured for BTreeSet<T> {
    fn measure(&self) -> usize {
        self.len()
    }
}

impl<K, V> measured for HashMap<K, V> {
    fn measure(&self) -> usize {
        self.len()
    }
}

impl<K, V> measured for BTreeMap<K, V> {
    fn measure(&self) -> usize {
        self.len()
    }
}

impl<H: holds<N> + ?Sized, N> holds<N> for &H {
    fn holds_needle(&self, needle: &N) -> bool {
        (**self).holds_needle(needle)
    }
}

impl holds<&str> for str {
    fn holds_needle(&self, needle: &&str) -> bool {
        self.contains(*needle)
    }
}

impl holds<String> for str {
    fn holds_needle(&self, needle: &String) -> bool {
        self.contains(needle.as_str())
    }
}

impl holds<char> for str {
    fn holds_needle(&self, needle: &char) -> bool {
        self.contains(*needle)
    }
}

impl holds<&str> for String {
    fn holds_needle(&self, needle: &&str) -> bool {
        self.as_str().contains(*needle)
    }
}

impl holds<String> for String {
    fn holds_needle(&self, needle: &String) -> bool {
        self.as_str().contains(needle.as_str())
    }
}

impl holds<char> for String {
    fn holds_needle(&self, needle: &char) -> bool {
        self.as_str().contains(*needle)
    }
}

impl<T: PartialEq> holds<T> for [T] {
    fn holds_needle(&self, needle: &T) -> bool {
        self.contains(needle)
    }
}

impl<T: PartialEq> holds<T> for Vec<T> {
    fn holds_needle(&self, needle: &T) -> bool {
        self.contains(needle)
    }
}

impl<T: PartialEq, const N: usize> holds<T> for [T; N] {
    fn holds_needle(&self, needle: &T) -> bool {
        self.contains(needle)
    }
}

// Lists of lines (`Vec<String>`) are common in tests, so they can be searched with a `&str`.
impl holds<&str> for [String] {
    fn holds_needle(&self, needle: &&str) -> bool {
        self.iter().any(|item| item == needle)
    }
}

impl holds<&str> for Vec<String> {
    fn holds_needle(&self, needle: &&str) -> bool {
        self.iter().any(|item| item == needle)
    }
}

impl<T: measured + Debug> subject<T> {
    /// The length is exactly `length` (bytes for text).
    #[track_caller]
    pub fn has_len(self, length: usize) -> Self {
        let actual = self.value.measure();
        if actual != length {
            self.fail(
                format!("a length of {length}"),
                format!("a length of {actual}: {}", show(&self.value)),
            );
        }
        self
    }

    /// There is nothing in it.
    #[track_caller]
    pub fn is_empty(self) -> Self {
        if self.value.measure() != 0 {
            self.fail_with_value("nothing in it");
        }
        self
    }

    /// There is something in it.
    #[track_caller]
    pub fn cannot_be_empty(self) -> Self {
        if self.value.measure() == 0 {
            self.fail("something in it", show(&self.value));
        }
        self
    }
}

impl<T: Debug> subject<T> {
    /// It holds `needle`: a piece of text, or an item of a list.
    #[track_caller]
    pub fn contains<N: Debug>(self, needle: N) -> Self
    where
        T: holds<N>,
    {
        if !self.value.holds_needle(&needle) {
            self.fail_with_value(format!("to contain {}", show(&needle)));
        }
        self
    }

    /// It does not hold `needle`.
    #[track_caller]
    pub fn cannot_contain<N: Debug>(self, needle: N) -> Self
    where
        T: holds<N>,
    {
        if self.value.holds_needle(&needle) {
            self.fail_with_value(format!("not to contain {}", show(&needle)));
        }
        self
    }
}
