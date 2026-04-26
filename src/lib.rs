// src/lib.rs
pub mod core;
pub mod edge;
pub mod error;
pub mod geom;
pub mod prelude;
pub mod topology;

pub use crate::core::buffer::DocBuffer;
use crate::edge::EdgeDetector;
pub use crate::error::DocQuadError;
use crate::geom::Quadrilateral;
use std::time::Instant;

/// 下采样阈值：对超过此像素数的图像自动缩小
const DOWNSAMPLE_THRESHOLD_PIXELS: u32 = 1024 * 768; // ~78万像素
/// 目标处理分辨率的最长边
const TARGET_LONG_EDGE: u32 = 1024;

/// 预过滤：最小周长比例（相对于图像最长边）
const MIN_PERIMETER_RATIO: f32 = 0.03;

/// 预过滤：最小包围盒面积比例（相对于图像面积）
const MIN_BBOX_AREA_RATIO: f32 = 0.005;

/// 预过滤：最大包围盒面积比例（排除全图噪声轮廓）
const MAX_BBOX_AREA_RATIO: f32 = 0.99;

/// 几何过滤：文档最小面积比例（相对于处理分辨率图像面积）
///
/// P1 修复：从 0.03 降至 0.01。
/// 原值 0.03 对应处理分辨率 23593px²，换算到原始分辨率（scale=0.25）约 37 万像素。
/// 若文档在照片中占比不足 3%（拍摄距离较远、文档偏小），轮廓会被面积过滤误杀。
/// 降至 0.01 后阈值为 7864px²（处理分辨率），对应原图约 12.6 万像素，更宽松。
const MIN_DOC_AREA_RATIO: f32 = 0.01;

/// 文档检测主入口。
pub fn find_document(buffer: &DocBuffer<'_>) -> Result<Option<Quadrilateral>, DocQuadError> {
    let start_total = Instant::now();

    let total_pixels = buffer.width * buffer.height;
    log::info!(
        "[Lib::find_document] - START: input={}x{}, total_pixels={}, stride={}",
        buffer.width,
        buffer.height,
        total_pixels,
        buffer.stride
    );

    // ── 阶段 1：下采样决策 ──────────────────────────────────────────────────
    let scale = if total_pixels > DOWNSAMPLE_THRESHOLD_PIXELS {
        let long_edge = buffer.width.max(buffer.height);
        TARGET_LONG_EDGE as f32 / long_edge as f32
    } else {
        1.0
    };

    log::debug!(
        "[Lib::find_document] - Downsample decision: total_pixels={}, threshold={}, scale={:.4}",
        total_pixels,
        DOWNSAMPLE_THRESHOLD_PIXELS,
        scale
    );

    let (proc_width, proc_height, proc_data) = if scale < 1.0 {
        let w = ((buffer.width as f32 * scale) as u32).max(3);
        let h = ((buffer.height as f32 * scale) as u32).max(3);

        // 【修复点】：使用双线性插值替代最近邻插值，保护边缘平滑性
        let data = downsample_bilinear(buffer, w, h);

        log::info!(
            "[Lib::find_document] - Downsampled {}x{} -> {}x{} (scale={:.4}) using bilinear interp",
            buffer.width,
            buffer.height,
            w,
            h,
            scale
        );
        (w, h, data)
    } else {
        // 无需下采样：紧凑化内存（去除 stride padding）
        let data = if buffer.stride == buffer.width {
            log::debug!("[Lib::find_document] - Contiguous memory, direct copy.");
            buffer.data[..(buffer.width * buffer.height) as usize].to_vec()
        } else {
            log::debug!(
                "[Lib::find_document] - Strided memory (stride={} > width={}), compacting rows.",
                buffer.stride,
                buffer.width
            );
            let view = buffer.as_array_view()?;
            let mut compact = Vec::with_capacity((buffer.width * buffer.height) as usize);
            for row in view.rows() {
                compact.extend(row.iter().copied());
            }
            compact
        };
        (buffer.width, buffer.height, data)
    };

    log::info!(
        "[Lib::find_document] - Processing resolution: {}x{}, proc_data_len={}",
        proc_width,
        proc_height,
        proc_data.len()
    );

    // ── 阶段 2：构建处理分辨率 DocBuffer ────────────────────────────────────
    let proc_buffer = DocBuffer::new(&proc_data, proc_width, proc_height, proc_width)?;

    // ── 阶段 3：边缘检测（含形态学闭运算）──────────────────────────────────
    log::info!("[Lib::find_document] - Stage 3: Edge detection (Canny + morphological close).");
    let mut detector = EdgeDetector::new(proc_width as usize, proc_height as usize)?;
    let edges = detector.detect(&proc_buffer)?;

    // 统计最终边缘像素数
    let edge_pixel_count = edges.iter().filter(|&&v| v == 255).count();
    let edge_density = edge_pixel_count as f32 / (proc_width * proc_height) as f32 * 100.0;
    log::info!(
        "[Lib::find_document] - Stage 3 result: edge_pixels={}, density={:.2}%",
        edge_pixel_count,
        edge_density
    );

    if edge_density > 20.0 {
        log::warn!(
            "[Lib::find_document] - Edge density {:.2}% is very high (>20%). \
             Canny thresholds may be too low or morphological close over-connected noise.",
            edge_density
        );
    } else if edge_density < 0.1 {
        log::warn!(
            "[Lib::find_document] - Edge density {:.2}% is very low (<0.1%). \
             Canny thresholds may be too high, document edges may be missed.",
            edge_density
        );
    }

    // ── 阶段 4：轮廓提取 ────────────────────────────────────────────────────
    log::info!("[Lib::find_document] - Stage 4: Contour extraction.");
    let raw_contours =
        crate::topology::contour::ContourExtractor::extract(&edges, proc_width, proc_height);

    let raw_count = raw_contours.len();
    log::info!(
        "[Lib::find_document] - Stage 4 result: {} raw contours extracted.",
        raw_count
    );

    if raw_count == 0 {
        log::warn!(
            "[Lib::find_document] - No contours extracted. \
             Edge image may be empty or all edges are on image boundary."
        );
        log::info!(
            "[Lib::find_document] - Detection finished. Found=false. Total Elapsed: {}ms",
            start_total.elapsed().as_millis()
        );
        return Ok(None);
    }

    // ── 阶段 5：轮廓预过滤 ──────────────────────────────────────────────────
    let proc_area = (proc_width * proc_height) as f32;
    let long_edge_px = proc_width.max(proc_height) as f32;

    let min_perimeter = long_edge_px * MIN_PERIMETER_RATIO;
    let min_bbox_area = proc_area * MIN_BBOX_AREA_RATIO;
    let max_bbox_area = proc_area * MAX_BBOX_AREA_RATIO;

    log::info!(
        "[Lib::find_document] - Stage 5: Pre-filter params: \
         min_perimeter={:.1}px (ratio={:.2}), \
         min_bbox_area={:.0}px² (ratio={:.3}), \
         max_bbox_area={:.0}px² (ratio={:.2})",
        min_perimeter,
        MIN_PERIMETER_RATIO,
        min_bbox_area,
        MIN_BBOX_AREA_RATIO,
        max_bbox_area,
        MAX_BBOX_AREA_RATIO
    );

    let mut rejected_too_short = 0usize;
    let mut rejected_too_small_bbox = 0usize;
    let mut rejected_too_large_bbox = 0usize;

    // 记录被过滤轮廓的周长分布，辅助诊断阈值是否合理
    let mut perimeter_histogram = [0usize; 10]; // 每档 = long_edge_px * 0.05

    let filtered_contours: Vec<_> = raw_contours
        .into_iter()
        .filter(|contour| {
            // 快速点数过滤
            if contour.len() < 4 {
                rejected_too_short += 1;
                return false;
            }

            let perimeter = contour.len() as f32;

            // 统计周长分布（用于诊断日志）
            let bucket = ((perimeter / long_edge_px * 20.0) as usize).min(9);
            perimeter_histogram[bucket] += 1;

            // 周长过滤
            if perimeter < min_perimeter {
                rejected_too_short += 1;
                return false;
            }

            // 计算包围盒
            let (min_x, max_x, min_y, max_y) = contour.iter().fold(
                (f32::MAX, f32::MIN, f32::MAX, f32::MIN),
                |(mnx, mxx, mny, mxy), c| {
                    (mnx.min(c.x), mxx.max(c.x), mny.min(c.y), mxy.max(c.y))
                },
            );
            let bbox_area = (max_x - min_x) * (max_y - min_y);

            if bbox_area < min_bbox_area {
                rejected_too_small_bbox += 1;
                return false;
            }
            if bbox_area > max_bbox_area {
                rejected_too_large_bbox += 1;
                return false;
            }
            true
        })
        .collect();

    // 输出周长分布直方图，辅助判断 min_perimeter 阈值是否合理
    log::debug!(
        "[Lib::find_document] - Contour perimeter distribution \
         (bucket_width={:.1}px, 0~{:.1}px+):",
        long_edge_px * 0.05,
        long_edge_px * 0.5
    );
    for (i, &count) in perimeter_histogram.iter().enumerate() {
        if count > 0 {
            log::debug!(
                "[Lib::find_document] -   [{:.1}~{:.1}px]: {} contours",
                long_edge_px * 0.05 * i as f32,
                long_edge_px * 0.05 * (i + 1) as f32,
                count
            );
        }
    }

    log::info!(
        "[Lib::find_document] - Stage 5 result: {}/{} contours remain. \
         Rejected: too_short/small_perimeter={}, too_small_bbox={}, too_large_bbox={}",
        filtered_contours.len(),
        raw_count,
        rejected_too_short,
        rejected_too_small_bbox,
        rejected_too_large_bbox
    );

    if filtered_contours.is_empty() {
        log::warn!(
            "[Lib::find_document] - No contours survived pre-filter. \
             Current thresholds: min_perimeter={:.1}px, min_bbox_area={:.0}px². \
             All {} raw contours were too short/small. \
             This typically means edge continuity is poor — \
             check Canny output or increase morphological close radius.",
            min_perimeter,
            min_bbox_area,
            raw_count
        );
        log::info!(
            "[Lib::find_document] - Detection finished. Found=false. Total Elapsed: {}ms",
            start_total.elapsed().as_millis()
        );
        return Ok(None);
    }

    // ── 阶段 6：几何分析与筛选 ──────────────────────────────────────────────
    let min_area = proc_area * MIN_DOC_AREA_RATIO;

    log::info!(
        "[Lib::find_document] - Stage 6: Geometry analysis on {} contours. \
         min_area={:.0}px² (ratio={:.2}, proc_area={:.0}px²)",
        filtered_contours.len(),
        min_area,
        MIN_DOC_AREA_RATIO,
        proc_area
    );

    let mut candidates = Vec::new();
    let mut geom_rejected_simplify = 0usize;
    let mut geom_rejected_validate = 0usize;
    let mut geom_rejected_area = 0usize;

    for (idx, contour) in filtered_contours.into_iter().enumerate() {
        let contour_len = contour.len();

        // 计算该轮廓的包围盒，输出到日志辅助诊断
        let (min_x, max_x, min_y, max_y) = contour.iter().fold(
            (f32::MAX, f32::MIN, f32::MAX, f32::MIN),
            |(mnx, mxx, mny, mxy), c| (mnx.min(c.x), mxx.max(c.x), mny.min(c.y), mxy.max(c.y)),
        );
        let bbox_w = max_x - min_x;
        let bbox_h = max_y - min_y;

        log::debug!(
            "[Lib::find_document] - Geometry[{}]: contour_len={}, \
             bbox=[({:.0},{:.0})-({:.0},{:.0})] {:.0}x{:.0}px, \
             attempting simplify_to_quad.",
            idx,
            contour_len,
            min_x,
            min_y,
            max_x,
            max_y,
            bbox_w,
            bbox_h
        );

        let Some(simplified) =
                    crate::geom::simplify::GeometrySimplifier::simplify_to_quad(&contour)
        else {
            geom_rejected_simplify += 1;
            log::debug!(
                "[Lib::find_document] - Geometry[{}]: simplify_to_quad returned None \
                 (contour_len={}, bbox={:.0}x{:.0}px).",
                idx,
                contour_len,
                bbox_w,
                bbox_h
            );
            continue;
        };

        log::debug!(
            "[Lib::find_document] - Geometry[{}]: simplified to {} points, \
             attempting validate_and_score.",
            idx,
            simplified.0.len()
        );

        let Some((area, pts)) =
            crate::geom::validate::GeometryValidator::validate_and_score(&simplified)
        else {
            geom_rejected_validate += 1;
            log::debug!(
                "[Lib::find_document] - Geometry[{}]: validate_and_score returned None \
                 (contour_len={}).",
                idx,
                contour_len
            );
            continue;
        };

        // 面积还原到原始分辨率
        let original_area = area / (scale * scale);

        log::debug!(
            "[Lib::find_document] - Geometry[{}]: proc_area={:.0}px², \
             original_area={:.0}px², min_area={:.0}px², scale={:.4}",
            idx,
            area,
            original_area,
            min_area,
            scale
        );

        if area > min_area {
            let sorted_pts = crate::geom::transform::Transformer::sort_points(
                pts.map(|p| glam::Vec2::new(p.x / scale, p.y / scale)),
            );
            log::info!(
                "[Lib::find_document] - Geometry[{}]: ACCEPTED quad. \
                 original_area={:.0}px², \
                 points=[TL({:.1},{:.1}), TR({:.1},{:.1}), BR({:.1},{:.1}), BL({:.1},{:.1})]",
                idx,
                original_area,
                sorted_pts[0].x,
                sorted_pts[0].y,
                sorted_pts[1].x,
                sorted_pts[1].y,
                sorted_pts[2].x,
                sorted_pts[2].y,
                sorted_pts[3].x,
                sorted_pts[3].y,
            );
            candidates.push(Quadrilateral {
                points: sorted_pts,
                area: original_area,
                score: 1.0,
            });
        } else {
            geom_rejected_area += 1;
            log::debug!(
                "[Lib::find_document] - Geometry[{}]: rejected by area filter \
                 (proc_area={:.0} < min_area={:.0}).",
                idx,
                area,
                min_area
            );
        }
    }

    log::info!(
        "[Lib::find_document] - Stage 6 result: {} candidates accepted. \
         Rejected: simplify={}, validate={}, area_too_small={}",
        candidates.len(),
        geom_rejected_simplify,
        geom_rejected_validate,
        geom_rejected_area
    );

    // 选取面积最大的候选作为最终结果
    let result = candidates.into_iter().max_by(|a, b| {
        a.area
            .partial_cmp(&b.area)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    log::info!(
        "[Lib::find_document] - Detection finished. Found={}. Total Elapsed: {}ms",
        result.is_some(),
        start_total.elapsed().as_millis()
    );

    Ok(result)
}

/// 双线性插值下采样 (Bilinear Interpolation)
///
/// 替代原有的最近邻缩放，平滑融合源图像的相邻 4 个像素，
/// 能够极大缓解因下采样引发的高频锯齿 (Aliasing)，保障 Canny 边缘的连续性。
fn downsample_bilinear(buffer: &DocBuffer<'_>, target_w: u32, target_h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((target_w * target_h) as usize);
    let src_w = buffer.width;
    let src_h = buffer.height;
    let stride = buffer.stride as usize;
    let data = buffer.data;

    // 防止除以 0，确保边界至少有 2 个点可以插值
    let x_ratio = (src_w.saturating_sub(1)) as f32 / (target_w.max(2) - 1) as f32;
    let y_ratio = (src_h.saturating_sub(1)) as f32 / (target_h.max(2) - 1) as f32;

    for ty in 0..target_h {
        let gy = (ty as f32) * y_ratio;
        let y_floor = gy.floor() as usize;
        let y_ceil = (y_floor + 1).min(src_h as usize - 1);
        let y_weight = gy - y_floor as f32;
        let y_weight_inv = 1.0 - y_weight;

        let row_floor_offset = y_floor * stride;
        let row_ceil_offset = y_ceil * stride;

        for tx in 0..target_w {
            let gx = (tx as f32) * x_ratio;
            let x_floor = gx.floor() as usize;
            let x_ceil = (x_floor + 1).min(src_w as usize - 1);
            let x_weight = gx - x_floor as f32;
            let x_weight_inv = 1.0 - x_weight;

            // 获取周围的四个像素点 (Top-Left, Top-Right, Bottom-Left, Bottom-Right)
            let tl = data[row_floor_offset + x_floor] as f32;
            let tr = data[row_floor_offset + x_ceil] as f32;
            let bl = data[row_ceil_offset + x_floor] as f32;
            let br = data[row_ceil_offset + x_ceil] as f32;

            // X 轴线性插值
            let top = tl * x_weight_inv + tr * x_weight;
            let bottom = bl * x_weight_inv + br * x_weight;

            // Y 轴线性插值
            let val = top * y_weight_inv + bottom * y_weight;

            out.push(val.round() as u8);
        }
    }
    out
}
