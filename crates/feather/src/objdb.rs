use std::collections::HashMap;

use super::object::Object;

pub struct ObjDB<T: Object> {
    free_indexes: Vec<usize>,
    objects: Vec<Option<T>>,
    object_names: HashMap<String, usize>,
}

impl<T: Object> ObjDB<T> {
    pub fn new() -> Self {
        Self {
            free_indexes: Vec::new(),
            objects: Vec::new(),
            object_names: HashMap::new(),
        }
    }

    /// Adds an object and returns its handle. Every object gets its own handle, even when its
    /// name is already used; name lookups keep returning the object registered first.
    pub fn add(&mut self, mut object: T) -> usize {
        let handle = self.free_indexes.pop().unwrap_or(self.objects.len());
        if let Some(name) = object.get_name() {
            self.object_names.entry(name).or_insert(handle);
        }
        object.set_handle(handle);
        if handle >= self.objects.len() {
            self.objects.push(Some(object));
        } else {
            self.objects[handle] = Some(object);
        }

        handle
    }

    /// Removes an object. Invalid or already removed handles are ignored.
    pub fn remove(&mut self, handle: usize) {
        let Some(object) = self.objects.get_mut(handle).and_then(Option::take) else {
            return;
        };
        self.free_indexes.push(handle);

        let Some(name) = object.get_name() else {
            return;
        };
        if self.object_names.get(&name) != Some(&handle) {
            return;
        }
        // The name pointed at the removed object: hand it to another object with the same
        // name, if there is one.
        let other = self
            .objects
            .iter()
            .flatten()
            .find(|o| o.get_name().as_deref() == Some(name.as_str()))
            .map(|o| o.get_handle());
        match other {
            Some(other) => self.object_names.insert(name, other),
            None => self.object_names.remove(&name),
        };
    }

    pub fn get(&self, handle: usize) -> Option<&T> {
        self.objects.get(handle).and_then(Option::as_ref)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&T> {
        self.object_names
            .get(name)
            .and_then(|&handle| self.objects[handle].as_ref())
    }

    pub fn get_by_name_mut(&mut self, name: &str) -> Option<&mut T> {
        self.object_names
            .get(name)
            .and_then(|&handle| self.objects[handle].as_mut())
    }

    pub fn get_mut(&mut self, handle: usize) -> Option<&mut T> {
        self.objects.get_mut(handle).and_then(Option::as_mut)
    }

    /// How many objects there are (removed ones not counted).
    pub fn len(&self) -> usize {
        self.objects.len() - self.free_indexes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.into_iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.into_iter()
    }
}

impl<T: Object> Default for ObjDB<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Object> IntoIterator for ObjDB<T> {
    type Item = T;
    type IntoIter = std::iter::Flatten<std::vec::IntoIter<Option<T>>>;

    fn into_iter(self) -> Self::IntoIter {
        self.objects.into_iter().flatten()
    }
}

impl<'a, T: Object> IntoIterator for &'a ObjDB<T> {
    type Item = &'a T;
    type IntoIter = std::iter::Flatten<std::slice::Iter<'a, Option<T>>>;

    fn into_iter(self) -> Self::IntoIter {
        self.objects.iter().flatten()
    }
}

impl<'a, T: Object> IntoIterator for &'a mut ObjDB<T> {
    type Item = &'a mut T;
    type IntoIter = std::iter::Flatten<std::slice::IterMut<'a, Option<T>>>;

    fn into_iter(self) -> Self::IntoIter {
        self.objects.iter_mut().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestObject {
        name: Option<String>,
        handle: usize,
        value: i32,
    }

    impl Object for TestObject {
        fn get_name(&self) -> Option<String> {
            self.name.clone()
        }
        fn set_handle(&mut self, handle: usize) {
            self.handle = handle;
        }
        fn get_handle(&self) -> usize {
            self.handle
        }
    }

    impl TestObject {
        fn new(name: Option<&str>, value: i32) -> Self {
            Self {
                name: name.map(|s| s.to_string()),
                handle: 0,
                value,
            }
        }
    }

    #[test]
    fn len_counts_objects_not_removed() {
        let mut db = ObjDB::new();
        assert!(db.is_empty());
        let h1 = db.add(TestObject::new(Some("a"), 1));
        db.add(TestObject::new(None, 2));
        assert_eq!(db.len(), 2);
        db.remove(h1);
        db.remove(h1); // Already removed: ignored.
        db.remove(99); // Never added: ignored.
        assert_eq!(db.len(), 1);
        db.add(TestObject::new(None, 3)); // Reuses h1's slot.
        assert_eq!(db.len(), 2);
    }

    #[test]
    fn test_iteration() {
        let mut db = ObjDB::new();
        let h1 = db.add(TestObject::new(Some("a"), 1));
        let h2 = db.add(TestObject::new(Some("b"), 2));
        db.remove(h1);
        let h3 = db.add(TestObject::new(Some("c"), 3));

        // Iterate by reference
        let mut count = 0;
        let mut sum = 0;
        for obj in &db {
            count += 1;
            sum += obj.value;
        }
        assert_eq!(count, 2);
        assert_eq!(sum, 2 + 3);

        // Iterate by mutable reference
        for obj in &mut db {
            obj.value *= 2;
        }

        // iterate using iter()
        let sum_iter: i32 = db.iter().map(|o| o.value).sum();
        assert_eq!(sum_iter, 4 + 6);

        // Iterate consuming
        let mut sum = 0;
        for obj in db {
            sum += obj.value;
        }
        assert_eq!(sum, 4 + 6);
    }

    #[test]
    fn duplicate_names_get_separate_handles() {
        let mut db = ObjDB::new();
        let h1 = db.add(TestObject::new(Some("a"), 1));
        let h2 = db.add(TestObject::new(Some("a"), 2));

        assert_ne!(h1, h2);
        assert_eq!(db.get(h1).unwrap().value, 1);
        assert_eq!(db.get(h2).unwrap().value, 2);
        // Name lookup returns the object registered first.
        assert_eq!(db.get_by_name("a").unwrap().get_handle(), h1);
    }

    #[test]
    fn removing_named_object_moves_name_to_duplicate() {
        let mut db = ObjDB::new();
        let h1 = db.add(TestObject::new(Some("a"), 1));
        let h2 = db.add(TestObject::new(Some("a"), 2));

        db.remove(h1);
        assert_eq!(db.get_by_name("a").unwrap().get_handle(), h2);

        db.remove(h2);
        assert!(db.get_by_name("a").is_none());
    }

    #[test]
    fn removing_duplicate_keeps_name_of_first() {
        let mut db = ObjDB::new();
        let h1 = db.add(TestObject::new(Some("a"), 1));
        let h2 = db.add(TestObject::new(Some("a"), 2));

        db.remove(h2);
        assert_eq!(db.get_by_name("a").unwrap().get_handle(), h1);
    }

    #[test]
    fn named_object_reuses_free_index() {
        let mut db = ObjDB::new();
        let h1 = db.add(TestObject::new(None, 1));
        db.add(TestObject::new(None, 2));
        db.remove(h1);

        let h3 = db.add(TestObject::new(Some("c"), 3));
        assert_eq!(h3, h1);
    }

    #[test]
    fn invalid_handles_are_handled() {
        let mut db: ObjDB<TestObject> = ObjDB::new();
        assert!(db.get(42).is_none());
        assert!(db.get_mut(42).is_none());
        db.remove(42); // must not panic

        let h = db.add(TestObject::new(None, 1));
        db.remove(h);
        db.remove(h); // removing twice must not panic or duplicate the free index
        let a = db.add(TestObject::new(None, 2));
        let b = db.add(TestObject::new(None, 3));
        assert_ne!(a, b);
    }
}
