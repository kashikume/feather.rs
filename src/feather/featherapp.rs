use anyhow::Result;

pub trait FeatherApp {
    fn on_create(&mut self) -> Result<()>;
    fn on_render(&mut self) -> Result<()>;
    fn on_update(&mut self, time: usize) -> Result<()>;
    fn on_destroy(&mut self);
}
