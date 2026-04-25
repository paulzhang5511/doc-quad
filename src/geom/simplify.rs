// src/geom/simplify.rs
use geo::ConvexHull;
use geo::Simplify;
use geo_types::{Coord, LineString, Polygon};
use std::time::Instant;

/// 几何简化器
pub struct GeometrySimplifier;

impl GeometrySimplifier {
    /// 使用凸包 + Douglas-Peucker 算法简化轮廓，筛选出四边形候选。
    ///
    /// # 算法说明
    /// 1. 将输入坐标点集构造为 LineString 并主动闭合。
    /// 2. **新增**：对轮廓求凸包，消除锯齿噪声，使 D-P 更易收敛到 4 顶点。
    /// 3. 以多档容差对凸包执行 Douglas-Peucker 简化，寻找恰好为 5 点的结果。
    /// 4. 若多档容差均未能得到 5 点，则使用极值点法强制提取 4 个角点作为后备。
    ///
    /// # 凸包预处理的必要性
    /// 真实照片中文档边缘因透视变形、光照不均、纸张弯曲，Canny 产生带噪声的曲线段。
    /// 直接对锯齿轮廓执行 D-P，容差小时点数过多，容差大时直接跳过 5 点跌至 3 点，
    /// 无法在任何容差档位精确停在 5 点。凸包将轮廓预处理为凸多边形后，
    /// D-P 的简化路径更平滑，能稳定收敛到 4 顶点（5 点闭合 LineString）。
    ///
    /// # 闭合保证
    /// 输入轮廓若首尾不同，此处主动闭合，确保 geo::Simplify 对闭合轮廓的正确处理。
    pub fn simplify_to_quad(contour: Vec<Coord<f32>>) -> Option<LineString<f32>> {
        let start = Instant::now();

        if contour.len() < 4 {
            log::debug!(
                "[Geom::Simplify] - Contour too short (len={}), skipping.",
                contour.len()
            );
            return None;
        }

        let original_len = contour.len();

        // P1 修复：主动闭合轮廓，防止 ContourExtractor 输出未闭合时判断失效
        let mut coords = contour;
        if coords.first() != coords.last() {
            if let Some(&first) = coords.first() {
                coords.push(first);
            }
        }

        let line_string = LineString::new(coords);

        // 周长为零（退化轮廓）直接跳过
        let perimeter = geo::EuclideanLength::euclidean_length(&line_string);
        if perimeter < f32::EPSILON {
            log::debug!(
                "[Geom::Simplify] - Contour has zero perimeter (len={}), skipping.",
                original_len
            );
            return None;
        }

        // ★ P0 修复：凸包预处理
        // 对原始轮廓求凸包，将锯齿状噪声曲线转换为平滑凸多边形，
        // 大幅提升后续 Douglas-Peucker 简化到恰好 4 顶点的成功率。
        let polygon = Polygon::new(line_string.clone(), vec![]);
        let hull: LineString<f32> = polygon.convex_hull().exterior().clone();
        let hull_point_count = hull.0.len();

        log::debug!(
            "[Geom::Simplify] - Convex hull: original_len={}, hull_points={}, perimeter={:.1}",
            original_len,
            hull_point_count,
            perimeter
        );

        // 凸包退化检查（少于 4 点无法构成四边形）
        if hull_point_count < 5 {
            log::debug!(
                "[Geom::Simplify] - Hull degenerate (hull_points={} < 5), skipping.",
                hull_point_count
            );
            return None;
        }

        let hull_perimeter = geo::EuclideanLength::euclidean_length(&hull);

        // 多档容差依次尝试，从细粒度到粗粒度
        let epsilon_ratios: &[f32] = &[0.01, 0.02, 0.03, 0.05, 0.08, 0.12, 0.20, 0.30, 0.40];

        for &ratio in epsilon_ratios {
            let epsilon = hull_perimeter * ratio;
            let simplified = hull.simplify(&epsilon);
            let simplified_len = simplified.0.len();

            log::trace!(
                "[Geom::Simplify] - DP trial: ratio={:.0}%, epsilon={:.1}, simplified_points={}",
                ratio * 100.0,
                epsilon,
                simplified_len
            );

            if simplified_len == 5 {
                log::debug!(
                    "[Geom::Simplify] - Found quad via convex-hull+DP at epsilon_ratio={:.0}%. \
                     original_len={}, hull_points={}, Elapsed: {}µs",
                    ratio * 100.0,
                    original_len,
                    hull_point_count,
                    start.elapsed().as_micros()
                );
                return Some(simplified);
            }
        }

        log::debug!(
            "[Geom::Simplify] - DP failed to find 5-point result for hull (hull_points={}). \
             Falling back to extreme-point method. Elapsed: {}µs",
            hull_point_count,
            start.elapsed().as_micros()
        );

        // 后备策略：对凸包执行极值点法（比对原始轮廓更稳定，凸包已去除噪声）
        let result = Self::extract_quad_by_extremes(&hull);

        if result.is_some() {
            log::debug!(
                "[Geom::Simplify] - Extreme-point fallback SUCCEEDED (hull_points={}). Elapsed: {}µs",
                hull_point_count,
                start.elapsed().as_micros()
            );
        } else {
            log::debug!(
                "[Geom::Simplify] - Extreme-point fallback FAILED (hull_points={}). Elapsed: {}µs",
                hull_point_count,
                start.elapsed().as_micros()
            );
        }

        result
    }

    /// 极值点法：从轮廓中提取 4 个角点，构造近似四边形。
    ///
    /// # 算法说明
    /// 分别找到使 (x+y)、(x-y)、(-x+y)、(-x-y) 最大的点，
    /// 对应图像坐标系中的左上、右上、左下、右下四个极值方向。
    /// 这 4 个点对于矩形轮廓能精确定位四个角，对透视变形矩形也有良好近似。
    ///
    /// 返回的 LineString 包含 5 个点（4 顶点 + 重复起点），与 Douglas-Peucker 输出格式一致。
    ///
    /// # P1 修复
    /// 原退化检测使用 `f32::EPSILON` 精度判断 + 内层 `break` 导致计数不准确。
    /// 改为基于欧氏距离的阈值判断（5px），更符合像素坐标的实际精度需求。
    fn extract_quad_by_extremes(line_string: &LineString<f32>) -> Option<LineString<f32>> {
        let pts = &line_string.0;
        if pts.len() < 4 {
            return None;
        }

        // 分别寻找 4 个极值方向的代表点：
        // - min(x+y)  → 左上角（图像坐标系 y 向下）
        // - max(x-y)  → 右上角
        // - max(x+y)  → 右下角
        // - min(x-y)  → 左下角
        let tl = pts.iter().min_by(|a, b| {
            (a.x + a.y)
                .partial_cmp(&(b.x + b.y))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
        let tr = pts.iter().max_by(|a, b| {
            (a.x - a.y)
                .partial_cmp(&(b.x - b.y))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
        let br = pts.iter().max_by(|a, b| {
            (a.x + a.y)
                .partial_cmp(&(b.x + b.y))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
        let bl = pts.iter().min_by(|a, b| {
            (a.x - a.y)
                .partial_cmp(&(b.x - b.y))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;

        log::debug!(
            "[Geom::Simplify] - Extreme points: TL=({:.1},{:.1}), TR=({:.1},{:.1}), BR=({:.1},{:.1}), BL=({:.1},{:.1})",
            tl.x, tl.y, tr.x, tr.y, br.x, br.y, bl.x, bl.y
        );

        // P1 修复：改用欧氏距离阈值判断重合，替代原来的 f32::EPSILON + break 逻辑。
        // 原逻辑：内层 break 只跳出 j 循环，当 i=0 与 i=1 各有一个重合时，
        // count 只被减 1 而非 2，导致退化四边形漏检。
        // 新逻辑：任意两点距离 < 5px 即视为重合，直接拒绝。
        let corners = [tl, tr, br, bl];
        for i in 0..4 {
            for j in (i + 1)..4 {
                let dx = corners[i].x - corners[j].x;
                let dy = corners[i].y - corners[j].y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 5.0 {
                    log::warn!(
                        "[Geom::Simplify] - Extreme-point fallback produced degenerate quad: \
                         corners[{}]=({:.1},{:.1}) and corners[{}]=({:.1},{:.1}) are too close (dist={:.1}px < 5px).",
                        i, corners[i].x, corners[i].y,
                        j, corners[j].x, corners[j].y,
                        dist
                    );
                    return None;
                }
            }
        }

        // 构造闭合 LineString（5 点，首尾相同）
        let quad_coords = vec![*tl, *tr, *br, *bl, *tl];
        Some(LineString::new(quad_coords))
    }
}
