use std::collections::HashMap;

use super::uniform::{Uniform, UniformValue};

/// A list of uniforms with unique names, kept in the order they were first set.
///
/// Lookups by name go through a hash map, so `get`, `get_mut`, `set` and `contains` are O(1).
/// `remove` is O(n) because it keeps the order of the remaining uniforms.
#[derive(Clone, Debug, Default)]
pub struct Uniforms {
    items: Vec<Uniform>,
    /// Name -> index in `items`.
    index: HashMap<String, usize>,
}

impl Uniforms {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a uniform. If one with this name exists its value is replaced (the type may
    /// change), otherwise it is added at the end.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<UniformValue>) -> &mut Self {
        let name = name.into();
        let value = value.into();
        match self.index.get(&name) {
            Some(&i) => self.items[i].value = value,
            None => {
                self.index.insert(name.clone(), self.items.len());
                self.items.push(Uniform { name, value });
            }
        }
        self
    }

    pub fn get(&self, name: &str) -> Option<&UniformValue> {
        self.index.get(name).map(|&i| &self.items[i].value)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut UniformValue> {
        self.index.get(name).map(|&i| &mut self.items[i].value)
    }

    /// Removes a uniform and returns its value, if it existed.
    pub fn remove(&mut self, name: &str) -> Option<UniformValue> {
        let removed = self.index.remove(name)?;
        let uniform = self.items.remove(removed);
        // Uniforms after the removed one moved one place to the left.
        for i in self.index.values_mut() {
            if *i > removed {
                *i -= 1;
            }
        }
        Some(uniform.value)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.index.contains_key(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Uniform> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.index.clear();
    }
}

impl PartialEq for Uniforms {
    /// Equal when they hold the same uniforms in the same order.
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
    }
}

impl<'a> IntoIterator for &'a Uniforms {
    type Item = &'a Uniform;
    type IntoIter = std::slice::Iter<'a, Uniform>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feather::math::Vec4;

    /// Checks that the name index points at the right items.
    fn assert_index_consistent(uniforms: &Uniforms) {
        assert_eq!(uniforms.index.len(), uniforms.items.len());
        for (i, uniform) in uniforms.items.iter().enumerate() {
            assert_eq!(uniforms.index.get(&uniform.name), Some(&i));
        }
    }

    fn names(uniforms: &Uniforms) -> Vec<&str> {
        uniforms.iter().map(|u| u.name.as_str()).collect()
    }

    #[test]
    fn new_uniforms_is_empty() {
        let uniforms = Uniforms::new();
        assert!(uniforms.is_empty());
        assert_eq!(uniforms.len(), 0);
        assert_eq!(uniforms.get("x"), None);
    }

    #[test]
    fn set_and_get() {
        let mut uniforms = Uniforms::new();
        uniforms
            .set("roughness", 0.5f32)
            .set("color", Vec4::new(1.0, 0.0, 0.0, 1.0))
            .set("albedo", UniformValue::Texture(3));

        assert_eq!(uniforms.len(), 3);
        assert!(uniforms.contains("color"));
        assert_eq!(uniforms.get("roughness"), Some(&UniformValue::Float(0.5)));
        assert_eq!(
            uniforms.get("color"),
            Some(&UniformValue::Vec4(Vec4::new(1.0, 0.0, 0.0, 1.0)))
        );
        assert_eq!(uniforms.get("albedo"), Some(&UniformValue::Texture(3)));
        assert_eq!(uniforms.get("missing"), None);
        assert_index_consistent(&uniforms);
    }

    #[test]
    fn get_mut_changes_value() {
        let mut uniforms = Uniforms::new();
        uniforms.set("a", 1.0f32);

        *uniforms.get_mut("a").unwrap() = UniformValue::Float(2.0);

        assert_eq!(uniforms.get("a"), Some(&UniformValue::Float(2.0)));
        assert!(uniforms.get_mut("missing").is_none());
    }

    #[test]
    fn set_existing_name_replaces_value_in_place() {
        let mut uniforms = Uniforms::new();
        uniforms.set("a", 1.0f32).set("b", 2.0f32);
        uniforms.set("a", 5i32); // type may change

        assert_eq!(uniforms.len(), 2);
        assert_eq!(uniforms.get("a"), Some(&UniformValue::Int(5)));
        assert_eq!(names(&uniforms), ["a", "b"]);
        assert_index_consistent(&uniforms);
    }

    #[test]
    fn remove_keeps_order_and_lookups_of_the_rest() {
        let mut uniforms = Uniforms::new();
        uniforms
            .set("a", 1u32)
            .set("b", 2u32)
            .set("c", 3u32)
            .set("d", 4u32);

        assert_eq!(uniforms.remove("b"), Some(UniformValue::UInt(2)));
        assert_eq!(uniforms.remove("b"), None);

        assert!(!uniforms.contains("b"));
        assert_eq!(names(&uniforms), ["a", "c", "d"]);
        assert_eq!(uniforms.get("a"), Some(&UniformValue::UInt(1)));
        assert_eq!(uniforms.get("c"), Some(&UniformValue::UInt(3)));
        assert_eq!(uniforms.get("d"), Some(&UniformValue::UInt(4)));
        assert_index_consistent(&uniforms);
    }

    #[test]
    fn set_after_remove() {
        let mut uniforms = Uniforms::new();
        uniforms.set("a", 1u32).set("b", 2u32);
        uniforms.remove("a");
        uniforms.set("c", 3u32).set("a", 4u32);

        assert_eq!(names(&uniforms), ["b", "c", "a"]);
        assert_eq!(uniforms.get("a"), Some(&UniformValue::UInt(4)));
        assert_index_consistent(&uniforms);
    }

    #[test]
    fn iteration_keeps_insertion_order() {
        let mut uniforms = Uniforms::new();
        uniforms.set("z", 1u32).set("a", 2u32).set("m", 3u32);

        let names: Vec<_> = (&uniforms).into_iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["z", "a", "m"]);
    }

    #[test]
    fn clear() {
        let mut uniforms = Uniforms::new();
        uniforms.set("a", true);
        uniforms.clear();

        assert!(uniforms.is_empty());
        assert!(!uniforms.contains("a"));
        assert_index_consistent(&uniforms);
    }

    #[test]
    fn equality_depends_on_contents_and_order() {
        let mut a = Uniforms::new();
        a.set("x", 1u32).set("y", 2u32);
        let mut b = Uniforms::new();
        b.set("x", 1u32).set("y", 2u32);
        let mut c = Uniforms::new();
        c.set("y", 2u32).set("x", 1u32);

        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
