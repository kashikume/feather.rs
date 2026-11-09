use super::rect::Rect;
use std::{mem, slice::Iter};

#[derive(Debug)]
pub struct Rects {
    rects: Vec<Rect>,
}

impl Rects {
    pub fn new(w: usize, h: usize) -> Rects {
        Rects {
            rects: vec![Rect::from_wh(w, h)],
        }
    }

    pub fn len(&self) -> usize {
        self.rects.len()
    }
    pub fn all_rects(&self) -> Iter<Rect> {
        self.rects.iter()
    }

    pub fn find_best(&mut self, w: usize, h: usize) -> Option<Rect> {
        for best in 0..self.rects.len() {
            if self.rects[best].fits(w, h) {
                return Some(self.rects[best].clone());
            }
        }
        return None;
    }

    pub fn use_rect(&mut self, rc: Rect) {
        let mut old_rects = Vec::with_capacity(self.rects.len() + 6);
        mem::swap(&mut old_rects, &mut self.rects);

        for r in old_rects.into_iter() {
            if r.overlap(&rc) {
                let top = Rect {
                    x1: r.x1,
                    y1: r.y1,
                    x2: r.x2,
                    y2: rc.y1,
                };
                if !top.is_empty() && !self.rects.contains(&top) {
                    self.rects.push(top);
                }

                let bottom = Rect {
                    x1: r.x1,
                    y1: rc.y2,
                    x2: r.x2,
                    y2: r.y2,
                };
                if !bottom.is_empty() && !self.rects.contains(&bottom) {
                    self.rects.push(bottom);
                }

                let left = Rect {
                    x1: r.x1,
                    y1: r.y1,
                    x2: rc.x1,
                    y2: r.y2,
                };
                if !left.is_empty() && !self.rects.contains(&left) {
                    self.rects.push(left);
                }

                let right = Rect {
                    x1: rc.x2,
                    y1: r.y1,
                    x2: r.x2,
                    y2: r.y2,
                };
                if !right.is_empty() && !self.rects.contains(&right) {
                    self.rects.push(right);
                }
            } else {
                self.rects.push(r);
            }
        }
        self.rects
            .sort_by(|a, b| a.area().partial_cmp(&b.area()).unwrap());
    }
}
