#[derive(Default)]
pub struct IdGen {
    next_id: u64,
}

impl IdGen {
    pub fn next(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
