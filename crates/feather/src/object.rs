pub trait Object {
    fn get_name(&self) -> Option<String>;
    fn set_handle(&mut self, handle: usize);
    fn get_handle(&self) -> usize;
}