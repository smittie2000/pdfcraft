//! pdfcraft-geom — L0 geometry primitives shared by every PdfCraft crate.
//!
//! PDF user space is y-up with the origin at the bottom-left of the page; view space is y-down.
//! `PageRect` is always in PDF user space (points, 1/72 inch).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod perspective;

/// An axis-aligned rectangle in PDF user space (points).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PageRect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl PageRect {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0: x0.min(x1), y0: y0.min(y1), x1: x0.max(x1), y1: y0.max(y1) }
    }
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }
    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }
}

/// Page rotation in quarter turns, as in the PDF `/Rotate` key (normalised to 0/90/180/270).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Rotation {
    #[default]
    R0,
    R90,
    R180,
    R270,
}

impl Rotation {
    pub fn from_degrees(deg: i64) -> Self {
        match deg.rem_euclid(360) {
            90 => Self::R90,
            180 => Self::R180,
            270 => Self::R270,
            _ => Self::R0,
        }
    }
    pub fn swaps_axes(self) -> bool {
        matches!(self, Self::R90 | Self::R270)
    }
}

/// The box `[x0, y0, x1, y1]` around `points`. `None` when there are none or any coordinate is not
/// finite. A fold seeded with `f64::MAX` passes over NaN: an axis that is all NaN comes out as a
/// finite, inverted box, and one NaN corner shrinks the box. This refuses both. The box's width and
/// height are not checked: callers that need a finite extent check it.
pub fn bounds(points: impl IntoIterator<Item = [f64; 2]>) -> Option<[f64; 4]> {
    let finite = |[x, y]: [f64; 2]| x.is_finite() && y.is_finite();
    let mut points = points.into_iter();
    let [x, y] = points.next().filter(|&p| finite(p))?;
    points.try_fold([x, y, x, y], |[x0, y0, x1, y1], p @ [x, y]| finite(p).then(|| [x0.min(x), y0.min(y), x1.max(x), y1.max(y)]))
}

/// [`bounds`] of one `/QuadPoints` group `[x1, y1, x2, y2, x3, y3, x4, y4]`, corners in any order.
pub fn quad_bounds(quad: &[f64; 8]) -> Option<[f64; 4]> {
    bounds([[quad[0], quad[1]], [quad[2], quad[3]], [quad[4], quad[5]], [quad[6], quad[7]]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_normalises() {
        let r = PageRect::new(10.0, 20.0, 0.0, 5.0);
        assert_eq!((r.x0, r.y0, r.width(), r.height()), (0.0, 5.0, 10.0, 15.0));
    }

    #[test]
    fn rotation_normalises_negative_and_large_values() {
        assert_eq!(Rotation::from_degrees(-90), Rotation::R270);
        assert_eq!(Rotation::from_degrees(450), Rotation::R90);
        assert!(Rotation::R90.swaps_axes());
    }

    #[test]
    fn bounds_of_no_points_is_none() {
        assert_eq!(bounds([]), None);
    }

    #[test]
    fn bounds_of_one_point_is_that_point() {
        assert_eq!(bounds([[3.0, -4.0]]), Some([3.0, -4.0, 3.0, -4.0]));
    }

    #[test]
    fn any_non_finite_coordinate_gives_none() {
        let good = [[0.0, 0.0], [10.0, 0.0], [10.0, 5.0], [0.0, 5.0]];
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for i in 0..8 {
                let mut pts = good;
                pts[i / 2][i % 2] = bad;
                assert_eq!(bounds(pts), None, "{pts:?}");
            }
        }
        // All NaN, the case a fold seeded with f64::MAX turns into a finite, inverted box.
        assert_eq!(bounds([[f64::NAN; 2]; 4]), None);
    }

    #[test]
    fn every_corner_order_gives_the_same_box() {
        let c = [[10.0, 20.0], [20.0, 30.0], [0.0, 30.0], [10.0, 40.0]];
        let want = Some([0.0, 20.0, 20.0, 40.0]); // a square turned 45 degrees
        for a in 0..4 {
            for b in 0..4 {
                for d in 0..4 {
                    for e in 0..4 {
                        let idx = [a, b, d, e];
                        if (0..4).all(|k| idx.contains(&k)) {
                            assert_eq!(bounds(idx.map(|i| c[i])), want, "{idx:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_width_is_not_checked() {
        assert_eq!(bounds([[-1e308, 0.0], [1e308, 0.0]]), Some([-1e308, 0.0, 1e308, 0.0]));
    }

    #[test]
    fn quad_bounds_matches_bounds_of_its_corners() {
        let q = [10.0, 20.0, 20.0, 30.0, 0.0, 30.0, 10.0, 40.0];
        assert_eq!(quad_bounds(&q), bounds([[10.0, 20.0], [20.0, 30.0], [0.0, 30.0], [10.0, 40.0]]));
        let mut bad = q;
        bad[5] = f64::NAN;
        assert_eq!(quad_bounds(&bad), None);
    }

    proptest::proptest! {
        #[test]
        fn the_box_contains_every_finite_point(pts in proptest::collection::vec((-1e12f64..1e12, -1e12f64..1e12), 1..16)) {
            let pts: Vec<[f64; 2]> = pts.into_iter().map(|(x, y)| [x, y]).collect();
            let b = bounds(pts.iter().copied());
            proptest::prop_assert!(b.is_some());
            let [x0, y0, x1, y1] = b.unwrap_or_default();
            for [x, y] in pts {
                proptest::prop_assert!(x0 <= x && x <= x1 && y0 <= y && y <= y1);
            }
        }
    }
}
