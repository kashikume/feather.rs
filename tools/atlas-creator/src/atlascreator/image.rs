use std::path::PathBuf;

use super::rect::Rect;

#[derive(Debug)]
pub struct Image {
    pub file_name: PathBuf,
    pub name: String,
    pub rect: Rect,
}
