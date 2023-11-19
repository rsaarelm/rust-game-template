use std::{
    fmt,
    ops::Deref,
    str::FromStr,
    sync::{LazyLock, Mutex},
};

use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::HashMap;

// After https://matklad.github.io/2020/03/22/fast-simple-rust-interner.html

/// Immutable interned string.
///
/// Interned strings are the size of a machine word and can be copied and
/// tested for equality as cheaply as one. They deserialize and serialize as
/// regular strings.
#[derive(
    Copy,
    Clone,
    Default,
    Eq,
    PartialEq,
    Hash,
    Debug,
    DeserializeFromStr,
    SerializeDisplay,
)]
pub struct InString(usize);

impl InString {
    pub fn new(s: impl AsRef<str>) -> Self {
        InString(INTERNER.lock().unwrap().make(s))
    }

    pub fn as_str(&self) -> &str {
        INTERNER.lock().unwrap().get(self.0)
    }
}

impl From<&str> for InString {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for InString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for InString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for InString {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(InString::new(s))
    }
}

impl Deref for InString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

/// Optimized interned string container.
pub struct Interner {
    current_buffer: String,
    str_to_id: HashMap<&'static str, usize>,
    id_to_str: Vec<&'static str>,
    full_buffers: Vec<String>,
}

impl Interner {
    /// Initialize a new string interner backend with the given initial
    /// capacity.
    pub fn with_capacity(cap: usize) -> Interner {
        let cap = cap.next_power_of_two();
        let mut ret = Interner {
            current_buffer: String::with_capacity(cap),
            str_to_id: Default::default(),
            id_to_str: Default::default(),
            full_buffers: Default::default(),
        };

        // Set zero string to be "" so derived Default semantics work for
        // InString.
        let id = ret.make("");
        assert_eq!(id, 0);
        ret
    }

    /// Get the interned string ID corresponding to a string.
    ///
    /// Will allocate a new interned string if the argument hasn't been
    /// interned yet.
    pub fn make(&mut self, s: impl AsRef<str>) -> usize {
        let s: &str = s.as_ref();
        if let Some(&id) = self.str_to_id.get(&s) {
            id
        } else {
            let interned = unsafe { self.alloc(s) };
            let id = self.id_to_str.len();
            self.str_to_id.insert(interned, id);
            self.id_to_str.push(interned);

            debug_assert!(self.get(id) == interned);
            debug_assert!(self.make(s) == id);

            id
        }
    }

    /// Get the string reference from an interned id.
    ///
    /// Will panic on you if you somehow manufactured a non-registered Symbol
    /// value.
    pub fn get(&self, sym: usize) -> &'static str {
        self.id_to_str[sym]
    }

    unsafe fn alloc(&mut self, s: &str) -> &'static str {
        let cap = self.current_buffer.capacity();

        // Went over capacity of current buffer. Push current buffer into the
        // list of full buffers, so we don't need to move the memory around
        // (current buffer stays unchanged so all the slices pointing to it
        // are still valid). Create a new, bigger buffer that will fit
        // the string.
        if cap < self.current_buffer.len() + s.len() {
            let new_buffer = String::with_capacity(
                (cap.max(s.len()) + 1).next_power_of_two(),
            );
            let old_buffer =
                std::mem::replace(&mut self.current_buffer, new_buffer);
            self.full_buffers.push(old_buffer);
        }

        // The current buffer must have enough space for the new string at
        // this point.
        debug_assert!(
            self.current_buffer.capacity() - self.current_buffer.len()
                >= s.len()
        );

        let interned = {
            let start = self.current_buffer.len();
            self.current_buffer.push_str(s);
            &self.current_buffer[start..]
        };

        &*(interned as *const str)
    }
}

impl Default for Interner {
    fn default() -> Self {
        // Tiny initial buffers will get churned out very fast, so let's start
        // out with some sizable initial one.
        Interner::with_capacity(4096)
    }
}

static INTERNER: LazyLock<Mutex<Interner>> = LazyLock::new(Default::default);

#[cfg(test)]
mod test {
    use super::*;
    use quickcheck_macros::quickcheck;

    #[quickcheck]
    fn instring_alloc(s: String) -> bool {
        // Spam a bunch of strings into the interner and see that they all
        // come out fine.
        let is = InString::from(s.as_str());
        is.as_str() == s.as_str()
    }

    #[test]
    fn default() {
        assert_eq!(InString::default().as_str(), "");
    }
}
