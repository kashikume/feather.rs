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

    pub fn add(&mut self, mut object: T) -> usize {
        let handle = if let Some(name) = object.get_name() {
            if self.object_names.contains_key(&name) {
                *self.object_names.get(&name).unwrap()
            } else {
                self.free_indexes.pop().unwrap_or(self.objects.len())
            }
        } else if !self.free_indexes.is_empty() {
            self.free_indexes.pop().unwrap()
        } else {
            self.objects.len()
        };
        if let Some(name) = object.get_name() {
            self.object_names.insert(name, handle);
        }
        object.set_handle(handle);
        if handle >= self.objects.len() {
            self.objects.push(Some(object));
        } else {
            self.objects[handle] = Some(object);
        }

        handle
    }

    pub fn remove(&mut self, handle: usize) {
        if let Some(object) = self.objects[handle].take() {
            self.objects[handle] = None;
            self.free_indexes.push(handle);
            if let Some(name) = object.get_name() {
                self.object_names.remove(&name);
            }
        }
    }

    pub fn get(&self, handle: usize) -> Option<&T> {
        self.objects[handle].as_ref()
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
        self.objects[handle].as_mut()
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
}
