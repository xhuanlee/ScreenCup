use serde::{Deserialize, Serialize};

/// A rectangle in logical (points / DIPs) coordinates.
/// Origin is the top-left corner of the referenced target (display or overlay window).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }
    }

    /// Build a rect from two arbitrary corner points, normalising to positive size.
    pub fn from_points(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            x: x1.min(x2),
            y: y1.min(y2),
            width: (x2 - x1).abs(),
            height: (y2 - y1).abs(),
        }
    }

    #[inline]
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    #[inline]
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    #[inline]
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    #[inline]
    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    /// A usable selection has positive, non-trivial dimensions.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.width >= 8.0 && self.height >= 8.0
    }

    /// Clamp origin and size so the rect fits inside `bounds`.
    pub fn clamp_to(&self, bounds: &Rect) -> Self {
        let x = self.x.clamp(bounds.x, bounds.right());
        let y = self.y.clamp(bounds.y, bounds.bottom());
        let width = self.width.min(bounds.right() - x).max(0.0);
        let height = self.height.min(bounds.bottom() - y).max(0.0);
        Self { x, y, width, height }
    }

    /// Translate the rect so it becomes relative to `origin`'s coordinate space.
    pub fn relative_to(&self, origin: &Rect) -> Self {
        Self {
            x: self.x - origin.x,
            y: self.y - origin.y,
            width: self.width,
            height: self.height,
        }
    }

    /// Round to integer pixel boundaries for cleaner encoding.
    pub fn rounded(&self) -> Self {
        Self {
            x: self.x.round(),
            y: self.y.round(),
            width: self.width.round(),
            height: self.height.round(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_points_normalises() {
        assert_eq!(
            Rect::from_points(100.0, 50.0, 10.0, 200.0),
            Rect::new(10.0, 50.0, 90.0, 150.0)
        );
        assert_eq!(
            Rect::from_points(0.0, 0.0, 800.0, 600.0),
            Rect::new(0.0, 0.0, 800.0, 600.0)
        );
    }

    #[test]
    fn validity_threshold() {
        assert!(!Rect::new(0.0, 0.0, 7.0, 100.0).is_valid());
        assert!(!Rect::new(0.0, 0.0, 100.0, 7.9).is_valid());
        assert!(Rect::new(0.0, 0.0, 8.0, 8.0).is_valid());
    }

    #[test]
    fn clamp_to_bounds() {
        let bounds = Rect::new(0.0, 0.0, 1000.0, 800.0);
        assert_eq!(
            Rect::new(-50.0, -50.0, 2000.0, 2000.0).clamp_to(&bounds),
            Rect::new(0.0, 0.0, 1000.0, 800.0)
        );
        assert_eq!(
            Rect::new(900.0, 700.0, 500.0, 500.0).clamp_to(&bounds),
            Rect::new(900.0, 700.0, 100.0, 100.0)
        );
    }

    #[test]
    fn relative_to_origin() {
        let monitor = Rect::new(-1920.0, 0.0, 1920.0, 1080.0);
        let selection = Rect::new(-1920.0, 100.0, 500.0, 400.0);
        assert_eq!(
            selection.relative_to(&monitor),
            Rect::new(0.0, 100.0, 500.0, 400.0)
        );
    }

    #[test]
    fn center_and_area() {
        let r = Rect::new(10.0, 20.0, 100.0, 40.0);
        assert_eq!(r.center(), (60.0, 40.0));
        assert_eq!(r.area(), 4000.0);
    }
}
