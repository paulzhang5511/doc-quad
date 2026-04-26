// src/geom/simplify.rs
use glam::Vec2;

pub struct GeometrySimplifier;

impl GeometrySimplifier {
    /// 简化轮廓为四边形（恰好 4 个顶点，闭合后返回 5 个点）
    pub fn simplify_to_quad(contour: &[Vec2]) -> Option<(Vec<Vec2>, bool)> {
        if contour.len() < 4 {
            return None;
        }

        // 去重
        let mut pts: Vec<Vec2> = Vec::with_capacity(contour.len());
        for &v in contour {
            if pts.is_empty() || pts.last().unwrap().distance_squared(v) > 1e-4 {
                pts.push(v);
            }
        }

        if pts.len() < 4 {
            return None;
        }

        let is_initially_closed = pts.first().unwrap().distance_squared(*pts.last().unwrap()) < 1e-4;
        if is_initially_closed {
            pts.pop();
        }

        if pts.len() < 4 {
            return None;
        }

        if pts.len() == 4 {
            let mut result = pts.clone();
            result.push(result[0]); 
            return Some((result, true));
        }

        // Visvalingam-Whyatt 核心坍缩逻辑
        while pts.len() > 4 {
            let mut min_area = f32::MAX;
            let mut min_idx = 0;

            let n = pts.len();
            for i in 0..n {
                let prev = pts[(i + n - 1) % n];
                let curr = pts[i];
                let next = pts[(i + 1) % n];

                let area = Self::triangle_area(prev, curr, next);
                if area < min_area {
                    min_area = area;
                    min_idx = i;
                }
            }

            pts.remove(min_idx);
        }

        pts.push(pts[0]);
        Some((pts, true))
    }

    #[inline]
    fn triangle_area(a: Vec2, b: Vec2, c: Vec2) -> f32 {
        ((a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y)) / 2.0).abs()
    }
}