// src/geom/validate.rs
use glam::Vec2;

pub struct GeometryValidator;

impl GeometryValidator {
    pub fn validate_and_score(simplified: &(Vec<Vec2>, bool)) -> Option<(f32, [Vec2; 4])> {
        let pts = &simplified.0;
        let is_closed = simplified.1;

        if !is_closed || pts.len() != 5 {
            log::debug!(
                "[Geom::Validate] - Failed base check: is_closed={}, len={} (expected closed, len=5).",
                is_closed, pts.len()
            );
            return None;
        }

        let p = [pts[0], pts[1], pts[2], pts[3]];

        let cross = [
            Self::cross_product(p[0], p[1], p[2]),
            Self::cross_product(p[1], p[2], p[3]),
            Self::cross_product(p[2], p[3], p[0]),
            Self::cross_product(p[3], p[0], p[1]),
        ];

        if cross.iter().any(|&c| c.abs() < 1.0) {
            log::warn!(
                "[Geom::Validate] - Quadrilateral failed collinear test (degenerate polygon). Cross products: {:?}",
                cross
            );
            return None;
        }

        let non_zero: Vec<f32> = cross.iter().copied().filter(|&c| c != 0.0).collect();
        if non_zero.is_empty() {
            log::warn!("[Geom::Validate] - Quadrilateral completely degenerate (all cross products zero).");
            return None;
        }

        let positive_count = non_zero.iter().filter(|&&c| c > 0.0).count();
        let negative_count = non_zero.len() - positive_count;
        let minority_count = positive_count.min(negative_count);

        if minority_count > 0 {
            log::warn!(
                "[Geom::Validate] - Quadrilateral failed convexity test (concave or self-intersecting detected). \
                 Positive crosses: {}, Negative crosses: {}",
                positive_count, negative_count
            );
            return None;
        }

        let area = Self::polygon_area(&p);
        Some((area, p))
    }

    #[inline]
    fn cross_product(p0: Vec2, p1: Vec2, p2: Vec2) -> f32 {
        (p1.x - p0.x) * (p2.y - p1.y) - (p1.y - p0.y) * (p2.x - p1.x)
    }

    #[inline]
    fn polygon_area(pts: &[Vec2; 4]) -> f32 {
        let mut area = 0.0;
        for i in 0..4 {
            let j = (i + 1) % 4;
            area += pts[i].x * pts[j].y - pts[j].x * pts[i].y;
        }
        (area / 2.0).abs()
    }
}