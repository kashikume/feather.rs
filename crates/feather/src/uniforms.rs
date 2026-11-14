use super::uniform::{Uniform, UniformValue};
use super::uniformname::{UniformKey, UniformName};

/// A list of uniforms with unique names, kept in the order they were first set.
///
/// Names are interned (`UniformName`), and lists are short, so lookups scan the list comparing
/// ids: no hashing and no allocation. Cloning a list copies plain data. `remove` keeps the
/// order of the remaining uniforms.
///
/// Uniforms are set by name (`&str`, `String`) or by `UniformName`, and looked up by either
/// (`UniformKey`); looking up a name never used anywhere finds nothing without adding it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Uniforms {
    /// Equal lists hold the same uniforms in the same order.
    items: Vec<Uniform>,
}

impl Uniforms {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a uniform. If one with this name exists its value is replaced (the type may
    /// change), otherwise it is added at the end.
    pub fn set(
        &mut self,
        name: impl Into<UniformName>,
        value: impl Into<UniformValue>,
    ) -> &mut Self {
        let name = name.into();
        let value = value.into();
        match self.position(name) {
            Some(i) => self.items[i].value = value,
            None => self.items.push(Uniform { name, value }),
        }
        self
    }

    pub fn get(&self, name: impl UniformKey) -> Option<&UniformValue> {
        let i = self.position(name.uniform_name()?)?;
        Some(&self.items[i].value)
    }

    pub fn get_mut(&mut self, name: impl UniformKey) -> Option<&mut UniformValue> {
        let i = self.position(name.uniform_name()?)?;
        Some(&mut self.items[i].value)
    }

    /// Removes a uniform and returns its value, if it existed.
    pub fn remove(&mut self, name: impl UniformKey) -> Option<UniformValue> {
        let i = self.position(name.uniform_name()?)?;
        Some(self.items.remove(i).value)
    }

    pub fn contains(&self, name: impl UniformKey) -> bool {
        name.uniform_name()
            .is_some_and(|name| self.position(name).is_some())
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
    }

    /// Makes this list a copy of `other`, reusing its storage.
    pub fn clone_from_uniforms(&mut self, other: &Uniforms) {
        self.items.clear();
        self.items.extend_from_slice(&other.items);
    }

    fn position(&self, name: UniformName) -> Option<usize> {
        self.items.iter().position(|u| u.name == name)
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
    use crate::math::Vec4;

    /// Checks that no name is in the list twice.
    fn assert_index_consistent(uniforms: &Uniforms) {
        let unique: std::collections::HashSet<_> = uniforms.iter().map(|u| u.name).collect();
        assert_eq!(unique.len(), uniforms.len());
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
            .set("albedo", UniformValue::UInt(3));

        assert_eq!(uniforms.len(), 3);
        assert!(uniforms.contains("color"));
        assert_eq!(uniforms.get("roughness"), Some(&UniformValue::Float(0.5)));
        assert_eq!(
            uniforms.get("color"),
            Some(&UniformValue::Vec4(Vec4::new(1.0, 0.0, 0.0, 1.0)))
        );
        assert_eq!(uniforms.get("albedo"), Some(&UniformValue::UInt(3)));
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

    #[test]
    fn names_and_ids_find_the_same_uniform() {
        let mut uniforms = Uniforms::new();
        let id = UniformName::new("uniforms_test_by_id");
        uniforms.set(id, 1.0f32);
        assert_eq!(
            uniforms.get("uniforms_test_by_id"),
            Some(&UniformValue::Float(1.0))
        );
        uniforms.set("uniforms_test_by_id", 2.0f32);
        assert_eq!(uniforms.len(), 1);
        assert_eq!(uniforms.get(id), Some(&UniformValue::Float(2.0)));
        let name = "uniforms_test_by_id".to_string();
        assert!(uniforms.contains(&name));
    }

    #[test]
    fn looking_up_unknown_names_does_not_add_them() {
        let uniforms = Uniforms::new();
        assert_eq!(uniforms.get("uniforms_test_never_set"), None);
        assert!(!uniforms.contains("uniforms_test_never_set"));
        assert_eq!(UniformName::find("uniforms_test_never_set"), None);
    }

    #[test]
    fn clone_from_uniforms_copies() {
        let mut a = Uniforms::new();
        a.set("x", 1u32);
        let mut b = Uniforms::new();
        b.set("y", 2u32).set("z", 3u32);
        b.clone_from_uniforms(&a);
        assert_eq!(a, b);
    }
}
