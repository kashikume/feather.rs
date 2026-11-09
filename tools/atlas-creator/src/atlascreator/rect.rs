#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
    pub x1: usize,
    pub y1: usize,
    pub x2: usize,
    pub y2: usize,
}

impl Rect {
    pub fn from_wh(w: usize, h: usize) -> Rect {
        Rect {
            x1: 0,
            y1: 0,
            x2: w,
            y2: h,
        }
    }

    pub fn width(&self) -> usize {
        if self.x2 > self.x1 {
            self.x2 - self.x1
        } else {
            0
        }
    }
    pub fn height(&self) -> usize {
        if self.y2 > self.y1 {
            self.y2 - self.y1
        } else {
            0
        }
    }
    pub fn is_empty(&self) -> bool {
        self.width() == 0 || self.height() == 0
    }
    pub fn area(&self) -> usize {
        self.width() * self.height()
    }
    pub fn fits(&self, w: usize, h: usize) -> bool {
        w <= self.width() && h <= self.height()
    }

    pub fn move_to(&mut self, x: usize, y: usize) {
        self.x2 += x - self.x1;
        self.y2 += y - self.y1;
        self.x1 = x;
        self.y1 = y;
    }

    pub fn overlap(&self, other: &Rect) -> bool {
        // If one rectangle is on left side of other
        if self.x1 >= other.x2 || other.x1 >= self.x2 {
            false
        }
        // If one rectangle is above other
        else if self.y2 <= other.y1 || other.y2 <= self.y1 {
            false
        } else {
            true
        }
    }

    pub fn expand(&mut self, size: usize) {
        self.x1 -= size;
        self.x2 += size;
        self.y1 -= size;
        self.y2 += size;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_create_from_wh() {
        let rc = Rect::from_wh(20, 30);
        assert_eq!(rc.width(), 20);
        assert_eq!(rc.height(), 30);
    }

    #[test]
    fn test_is_empty() {
        assert_eq!(Rect::from_wh(0, 10).is_empty(), true);
        assert_eq!(Rect::from_wh(10, 0).is_empty(), true);
        assert_eq!(Rect::from_wh(0, 0).is_empty(), true);
        assert_eq!(Rect::from_wh(10, 10).is_empty(), false);

        assert_eq!(
            Rect {
                x1: 10,
                x2: 20,
                y1: 10,
                y2: 20
            }
            .is_empty(),
            false
        );
        assert_eq!(
            Rect {
                x1: 10,
                x2: 5,
                y1: 10,
                y2: 20
            }
            .is_empty(),
            true
        );
        assert_eq!(
            Rect {
                x1: 10,
                x2: 20,
                y1: 10,
                y2: 5
            }
            .is_empty(),
            true
        );

        assert_eq!(
            Rect {
                x1: 10,
                x2: 10,
                y1: 10,
                y2: 20
            }
            .is_empty(),
            true
        );
        assert_eq!(
            Rect {
                x1: 10,
                x2: 20,
                y1: 10,
                y2: 10
            }
            .is_empty(),
            true
        );
    }

    #[test]
    fn test_move_to() {
        let mut r = Rect {
            x1: 10,
            x2: 20,
            y1: 10,
            y2: 20,
        };
        assert_eq!(r.width(), 10);
        assert_eq!(r.height(), 10);

        r.move_to(20, 20);
        assert_eq!(r.width(), 10);
        assert_eq!(r.height(), 10);
        assert_eq!(r.x1, 20);
        assert_eq!(r.y1, 20);
    }

    #[test]
    fn test_overlap() {
        let rc = Rect {
            x1: 10,
            y1: 10,
            x2: 20,
            y2: 20,
        };

        assert_eq!(
            Rect {
                x1: 0,
                y1: 0,
                x2: 30,
                y2: 30
            }
            .overlap(&rc),
            true
        );
        assert_eq!(
            Rect {
                x1: 10,
                y1: 10,
                x2: 20,
                y2: 20
            }
            .overlap(&rc),
            true
        );
        assert_eq!(
            Rect {
                x1: 0,
                y1: 0,
                x2: 15,
                y2: 30
            }
            .overlap(&rc),
            true
        );
        assert_eq!(
            Rect {
                x1: 15,
                y1: 0,
                x2: 30,
                y2: 30
            }
            .overlap(&rc),
            true
        );
        assert_eq!(
            Rect {
                x1: 0,
                y1: 0,
                x2: 30,
                y2: 15
            }
            .overlap(&rc),
            true
        );
        assert_eq!(
            Rect {
                x1: 0,
                y1: 15,
                x2: 30,
                y2: 30
            }
            .overlap(&rc),
            true
        );

        assert_eq!(
            Rect {
                x1: 0,
                y1: 0,
                x2: 5,
                y2: 5
            }
            .overlap(&rc),
            false
        );
        assert_eq!(
            Rect {
                x1: 20,
                y1: 0,
                x2: 25,
                y2: 5
            }
            .overlap(&rc),
            false
        );
    }

    #[test]
    fn test_area() {
        assert_eq!(Rect::from_wh(4, 5).area(), 20);
        assert_eq!(Rect::from_wh(0, 5).area(), 0);
        let inverted = Rect {
            x1: 10,
            y1: 0,
            x2: 5,
            y2: 5,
        };
        assert_eq!(inverted.area(), 0);
    }

    #[test]
    fn test_fits() {
        let rc = Rect::from_wh(10, 20);
        assert!(rc.fits(10, 20));
        assert!(rc.fits(1, 1));
        assert!(rc.fits(0, 0));
        assert!(!rc.fits(11, 20));
        assert!(!rc.fits(10, 21));
    }

    #[test]
    fn test_expand() {
        let mut rc = Rect {
            x1: 5,
            y1: 6,
            x2: 10,
            y2: 12,
        };
        rc.expand(2);
        assert_eq!(
            rc,
            Rect {
                x1: 3,
                y1: 4,
                x2: 12,
                y2: 14
            }
        );
        assert_eq!((rc.width(), rc.height()), (9, 10));
    }

    #[test]
    fn test_overlap_is_symmetric() {
        let a = Rect {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
        };
        let b = Rect {
            x1: 9,
            y1: 9,
            x2: 20,
            y2: 20,
        };
        let touching = Rect {
            x1: 10,
            y1: 0,
            x2: 20,
            y2: 10,
        };
        assert!(a.overlap(&b) && b.overlap(&a));
        assert!(!a.overlap(&touching) && !touching.overlap(&a));
    }
}
