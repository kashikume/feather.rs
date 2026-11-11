//! Interned uniform names: each name gets a small id the first time it is used, so uniform
//! lists, shader layouts and draw packing compare integers instead of strings.
//!
//! There is one table for the whole process. Uniforms are created outside scenes (materials,
//! nodes) and shader layouts need the same ids, so ids must mean the same everywhere. The table
//! only grows: it holds the names a program uses, which are few.
//!
//! Ids depend on the order names are first used, so they differ between runs: never store them
//! or compare them across processes. Scene files keep names as strings.

use std::collections::HashMap;
use std::fmt;
use std::sync::{OnceLock, RwLock};

/// The id of a uniform name. Cheap to copy and compare; `as_str` gives the name back.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UniformName(u32);

#[derive(Default)]
struct Interner {
    ids: HashMap<&'static str, u32>,
    names: Vec<&'static str>,
}

fn table() -> &'static RwLock<Interner> {
    static TABLE: OnceLock<RwLock<Interner>> = OnceLock::new();
    TABLE.get_or_init(Default::default)
}

impl UniformName {
    /// The id of `name`, given out the first time it is asked for.
    pub fn new(name: &str) -> Self {
        if let Some(id) = Self::find(name) {
            return id;
        }
        let mut table = table().write().unwrap_or_else(|e| e.into_inner());
        // Another thread may have added it between the two locks.
        if let Some(&id) = table.ids.get(name) {
            return UniformName(id);
        }
        let id = table.names.len() as u32;
        // Names are never removed, so they can live as long as the program.
        let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
        table.names.push(name);
        table.ids.insert(name, id);
        UniformName(id)
    }

    /// The id of `name` if it has one. Doesn't add the name: for lookups, where a name that
    /// was never used can't be in any list.
    pub fn find(name: &str) -> Option<Self> {
        let table = table().read().unwrap_or_else(|e| e.into_inner());
        table.ids.get(name).map(|&id| UniformName(id))
    }

    /// The name.
    pub fn as_str(self) -> &'static str {
        let table = table().read().unwrap_or_else(|e| e.into_inner());
        table.names[self.0 as usize]
    }
}

impl fmt::Debug for UniformName {
    /// The name, not the number, so test failures and logs stay readable.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for UniformName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for UniformName {
    fn from(name: &str) -> Self {
        UniformName::new(name)
    }
}

impl From<&String> for UniformName {
    fn from(name: &String) -> Self {
        UniformName::new(name)
    }
}

impl From<String> for UniformName {
    fn from(name: String) -> Self {
        UniformName::new(&name)
    }
}

impl PartialEq<str> for UniformName {
    fn eq(&self, other: &str) -> bool {
        UniformName::find(other) == Some(*self)
    }
}

impl PartialEq<&str> for UniformName {
    fn eq(&self, other: &&str) -> bool {
        *self == **other
    }
}

/// Something a uniform can be looked up by: a `UniformName`, or a name as a string. Looking up
/// a string never adds it to the table (see `UniformName::find`).
pub trait UniformKey {
    /// The id to look for; `None` when no uniform can have this name.
    fn uniform_name(&self) -> Option<UniformName>;
}

impl UniformKey for UniformName {
    fn uniform_name(&self) -> Option<UniformName> {
        Some(*self)
    }
}

impl UniformKey for str {
    fn uniform_name(&self) -> Option<UniformName> {
        UniformName::find(self)
    }
}

impl UniformKey for String {
    fn uniform_name(&self) -> Option<UniformName> {
        UniformName::find(self)
    }
}

impl<T: UniformKey + ?Sized> UniformKey for &T {
    fn uniform_name(&self) -> Option<UniformName> {
        (**self).uniform_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_name_same_id() {
        let a = UniformName::new("uniformname_test_a");
        let b = UniformName::new("uniformname_test_b");
        assert_eq!(UniformName::new("uniformname_test_a"), a);
        assert_eq!(UniformName::from("uniformname_test_a".to_string()), a);
        assert_ne!(a, b);
        assert_eq!(a.as_str(), "uniformname_test_a");
        assert_eq!(b.to_string(), "uniformname_test_b");
        assert_eq!(format!("{a:?}"), "\"uniformname_test_a\"");
        assert!(a == "uniformname_test_a");
        assert!(a != "uniformname_test_b");
    }

    #[test]
    fn find_does_not_add_names() {
        assert_eq!(UniformName::find("uniformname_test_never_added"), None);
        assert_eq!(UniformName::find("uniformname_test_never_added"), None);
        assert_eq!(("uniformname_test_never_added").uniform_name(), None);
        let added = UniformName::new("uniformname_test_added");
        assert_eq!(UniformName::find("uniformname_test_added"), Some(added));
        assert_eq!(("uniformname_test_added").uniform_name(), Some(added));
    }

    #[test]
    fn threads_agree_on_ids() {
        let names: Vec<String> = (0..50).map(|i| format!("uniformname_test_thread_{i}")).collect();
        let ids: Vec<Vec<UniformName>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| names.iter().map(UniformName::from).collect::<Vec<_>>()))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for other in &ids[1..] {
            assert_eq!(other, &ids[0]);
        }
        let unique: std::collections::HashSet<_> = ids[0].iter().collect();
        assert_eq!(unique.len(), names.len());
    }

    #[test]
    fn names_are_send_sync_copy() {
        fn check<T: Send + Sync + Copy>() {}
        check::<UniformName>();
    }
}
