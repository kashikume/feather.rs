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
            }
            else {
                self.free_indexes.pop().unwrap_or(self.objects.len())
            }
        }
        else if !self.free_indexes.is_empty() {
            self.free_indexes.pop().unwrap()
        }
        else {
            self.objects.len()
        };
        if let Some(name) = object.get_name() {
            self.object_names.insert(name, handle);
        }
        object.set_handle(handle);
        if handle >= self.objects.len() {
            self.objects.push(Some(object));
        }
        else {
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
        self.object_names.get(name).and_then(|&handle| self.objects[handle].as_ref())
    }

    pub fn get_by_name_mut(&mut self, name: &str) -> Option<&mut T> { 
        self.object_names.get(name).and_then(|&handle| self.objects[handle].as_mut())
    }

    pub fn get_mut(&mut self, handle: usize) -> Option<&mut T> {
        self.objects[handle].as_mut()
    }
}

impl<T: Object> Default for ObjDB<T> {
    fn default() -> Self {
        Self::new()
    }
}
