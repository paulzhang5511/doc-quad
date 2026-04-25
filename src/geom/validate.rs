// src/geom/validate.rs
use glam::Vec2;
use geo_types::LineString;

pub struct GeometryValidator;

impl GeometryValidator {
    /// 校验四边形是否近似为凸多边形并计算面积。
    ///
    /// # 凸性容差
    /// 允许最多 1 个叉积符号与主方向相反（容忍轻微透视变形），
    /// 但不允许超过 1 个，防止严重凹入四边形通过。
    pub fn validate_and_score(poly: &LineString<f32>) -> Option<(f32, [Vec2; 4])> {
        if poly.0.len() != 5 {
            return None;
        }

        let pts = [
            Vec2::new(poly.0[0].x, poly.0[0].y),
            Vec2::new(poly.0[1].x, poly.0[1].y),
            Vec2::new(poly.0[2].x, poly.0[2].y),
            Vec2::new(poly.0[3].x, poly.0[3].y),
        ];

        // 鞋带公式
        let mut area = 0.0f32;
        for i in 0..4 {
            let j = (i + 1) % 4;
            area += pts[i].x * pts[j].y;
            area -= pts[j].x * pts[i].y;
        }
        area = area.abs() * 0.5;

        if area < f32::EPSILON {
            return None;
        }

        // 计算各边叉积
        let mut crosses = [0.0f32; 4];
        for i in 0..4 {
            let v1 = pts[(i + 1) % 4] - pts[i];
            let v2 = pts[(i + 2) % 4] - pts[(i + 1) % 4];
            crosses[i] = v1.x * v2.y - v1.y * v2.x;
        }

        // 过滤接近零的叉积（共线边）
        let non_zero: Vec<f32> = crosses
            .iter()
            .filter(|&&c| c.abs() > area * 0.01) // 相对阈值：叉积绝对值 > 面积的 1%
            .copied()
            .collect();

        if non_zero.is_empty() {
            log::warn!("[Geom::Validate] - Quadrilateral is fully degenerate.");
            return None;
        }

        // 确定主方向（多数票）
        let positive_count = non_zero.iter().filter(|&&c| c > 0.0).count();
        let negative_count = non_zero.len() - positive_count;
        let minority_count = positive_count.min(negative_count);

        // 允许最多 1 个叉积与主方向相反（容忍轻微变形）
        if minority_count > 1 {
            log::warn!(
                "[Geom::Validate] - Quadrilateral failed convexity test ({} minority crosses).",
                minority_count
            );
            return None;
        }

        Some((area, pts))
    }
}
