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

#[cfg(test)]
mod test {
    use super::*;

    fn rect(x1: usize, y1: usize, x2: usize, y2: usize) -> Rect {
        Rect { x1, y1, x2, y2 }
    }

    fn free(rects: &Rects) -> Vec<Rect> {
        rects.all_rects().cloned().collect()
    }

    #[test]
    fn new_has_one_free_rect_covering_everything() {
        let rects = Rects::new(30, 20);
        assert_eq!(rects.len(), 1);
        assert_eq!(free(&rects), vec![rect(0, 0, 30, 20)]);
    }

    #[test]
    fn find_best_returns_a_fitting_free_rect() {
        let mut rects = Rects::new(30, 20);
        assert_eq!(rects.find_best(30, 20), Some(rect(0, 0, 30, 20)));
        assert_eq!(rects.find_best(5, 5), Some(rect(0, 0, 30, 20)));
        assert_eq!(rects.find_best(31, 1), None);
        assert_eq!(rects.find_best(1, 21), None);
    }

    #[test]
    fn using_a_corner_leaves_right_and_bottom() {
        let mut rects = Rects::new(10, 10);
        rects.use_rect(rect(0, 0, 4, 3));
        let mut left = free(&rects);
        left.sort_by_key(|r| (r.x1, r.y1, r.x2, r.y2));
        assert_eq!(left, vec![rect(0, 3, 10, 10), rect(4, 0, 10, 10)]);
    }

    #[test]
    fn using_the_middle_leaves_four_sides() {
        let mut rects = Rects::new(10, 10);
        rects.use_rect(rect(3, 4, 6, 7));
        let mut left = free(&rects);
        left.sort_by_key(|r| (r.x1, r.y1, r.x2, r.y2));
        assert_eq!(
            left,
            vec![
                rect(0, 0, 3, 10),
                rect(0, 0, 10, 4),
                rect(0, 7, 10, 10),
                rect(6, 0, 10, 10)
            ]
        );
    }

    #[test]
    fn using_everything_leaves_nothing() {
        let mut rects = Rects::new(10, 10);
        rects.use_rect(rect(0, 0, 10, 10));
        assert_eq!(rects.len(), 0);
        assert_eq!(rects.find_best(1, 1), None);
    }

    #[test]
    fn free_rects_never_overlap_used_ones_and_stay_inside() {
        let mut rects = Rects::new(64, 64);
        let mut used = Vec::new();
        for (w, h) in [
            (20, 10),
            (10, 30),
            (16, 16),
            (5, 40),
            (30, 5),
            (8, 8),
            (8, 8),
        ] {
            let mut placed = rects.find_best(w, h).unwrap();
            placed.x2 = placed.x1 + w;
            placed.y2 = placed.y1 + h;
            rects.use_rect(placed.clone());
            used.push(placed);

            for r in rects.all_rects() {
                assert!(!r.is_empty(), "empty free rect {r:?}");
                assert!(r.x2 <= 64 && r.y2 <= 64, "{r:?} outside the atlas");
                for u in &used {
                    assert!(!r.overlap(u), "free {r:?} overlaps used {u:?}");
                }
            }
        }
        for (i, a) in used.iter().enumerate() {
            for b in &used[i + 1..] {
                assert!(!a.overlap(b), "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn free_rects_are_sorted_by_area_and_unique() {
        let mut rects = Rects::new(50, 40);
        rects.use_rect(rect(10, 10, 20, 15));
        rects.use_rect(rect(30, 0, 35, 40));
        let areas: Vec<usize> = rects.all_rects().map(Rect::area).collect();
        assert!(areas.windows(2).all(|w| w[0] <= w[1]), "{areas:?}");
        let all = free(&rects);
        for (i, a) in all.iter().enumerate() {
            assert!(!all[i + 1..].contains(a), "{a:?} listed twice");
        }
    }

    #[test]
    fn find_best_prefers_the_smallest_fitting_rect() {
        let mut rects = Rects::new(10, 10);
        rects.use_rect(rect(0, 0, 8, 8));
        // Free: right 2x10 and bottom 10x2, both area 20; a 2x2 fits either. A 3x2 fits only
        // the bottom one.
        assert_eq!(rects.find_best(3, 2), Some(rect(0, 8, 10, 10)));
        assert_eq!(rects.find_best(2, 3), Some(rect(8, 0, 10, 10)));
    }
}
