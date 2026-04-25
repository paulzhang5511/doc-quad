// src/edge/morphology.rs
use std::time::Instant;

/// 形态学操作工具集
pub struct Morphology;

impl Morphology {
    /// 对二值边缘图执行膨胀操作（Dilation），用于填补 Canny 输出中的边缘断点。
    ///
    /// # 算法说明
    /// 使用 (2*radius+1)×(2*radius+1) 的矩形结构元素。
    /// 对每个像素，若其邻域内存在任意非零值，则输出置为 255。
    /// 膨胀后相邻的短边缘段会连通，使轮廓追踪能提取出完整的长轮廓。
    ///
    /// # 参数
    /// - `src`：输入二值图（0 或 255），长度为 width * height
    /// - `width`、`height`：图像尺寸
    /// - `radius`：膨胀半径（像素），推荐值 1~2
    ///
    /// # 返回
    /// 膨胀后的二值图，与输入等长
    pub fn dilate(src: &[u8], width: usize, height: usize, radius: usize) -> Vec<u8> {
        let start = Instant::now();

        let mut dst = vec![0u8; width * height];

        // 统计输入边缘像素数，用于日志对比
        let src_edge_count = src.iter().filter(|&&v| v > 0).count();

        for y in 0..height {
            for x in 0..width {
                // 检查以 (x,y) 为中心的邻域内是否存在边缘像素
                'outer: for dy in 0..=(2 * radius) {
                    for dx in 0..=(2 * radius) {
                        // 将无符号偏移转换为有符号坐标，避免下溢
                        let ny = y as isize + dy as isize - radius as isize;
                        let nx = x as isize + dx as isize - radius as isize;

                        // 边界裁剪
                        if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                            continue;
                        }

                        let nidx = ny as usize * width + nx as usize;
                        if src[nidx] > 0 {
                            dst[y * width + x] = 255;
                            break 'outer;
                        }
                    }
                }
            }
        }

        let dst_edge_count = dst.iter().filter(|&&v| v > 0).count();

        log::debug!(
            "[Edge::Morphology] - Dilate done: radius={}, src_edges={}, dst_edges={}, \
             growth_ratio={:.2}x. Elapsed: {}ms",
            radius,
            src_edge_count,
            dst_edge_count,
            if src_edge_count > 0 {
                dst_edge_count as f32 / src_edge_count as f32
            } else {
                0.0
            },
            start.elapsed().as_millis()
        );

        dst
    }

    /// 对二值边缘图执行腐蚀操作（Erosion），与膨胀配合实现闭运算（Close）。
    ///
    /// # 算法说明
    /// 使用 (2*radius+1)×(2*radius+1) 的矩形结构元素。
    /// 对每个像素，若其邻域内全部为非零值，则输出置为 255，否则为 0。
    /// 先膨胀后腐蚀（闭运算）可填补断点同时抑制膨胀带来的边缘增厚。
    pub fn erode(src: &[u8], width: usize, height: usize, radius: usize) -> Vec<u8> {
        let start = Instant::now();

        let mut dst = vec![0u8; width * height];
        let src_edge_count = src.iter().filter(|&&v| v > 0).count();

        for y in 0..height {
            for x in 0..width {
                let mut all_edge = true;

                'outer: for dy in 0..=(2 * radius) {
                    for dx in 0..=(2 * radius) {
                        let ny = y as isize + dy as isize - radius as isize;
                        let nx = x as isize + dx as isize - radius as isize;

                        // 边界像素视为非边缘（保守策略，避免边界膨胀）
                        if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                            all_edge = false;
                            break 'outer;
                        }

                        let nidx = ny as usize * width + nx as usize;
                        if src[nidx] == 0 {
                            all_edge = false;
                            break 'outer;
                        }
                    }
                }

                if all_edge {
                    dst[y * width + x] = 255;
                }
            }
        }

        let dst_edge_count = dst.iter().filter(|&&v| v > 0).count();

        log::debug!(
            "[Edge::Morphology] - Erode done: radius={}, src_edges={}, dst_edges={}. Elapsed: {}ms",
            radius,
            src_edge_count,
            dst_edge_count,
            start.elapsed().as_millis()
        );

        dst
    }

    /// 形态学闭运算（Close = Dilate → Erode），填补边缘断点并恢复边缘宽度。
    ///
    /// # 参数
    /// - `src`：输入二值边缘图
    /// - `width`、`height`：图像尺寸
    /// - `radius`：结构元素半径，推荐值 1（轻度填补）或 2（强力填补）
    pub fn close(src: &[u8], width: usize, height: usize, radius: usize) -> Vec<u8> {
        let start = Instant::now();

        log::debug!(
            "[Edge::Morphology] - Close operation start: {}x{}, radius={}",
            width,
            height,
            radius
        );

        let dilated = Self::dilate(src, width, height, radius);
        let closed = Self::erode(&dilated, width, height, radius);

        let src_edges = src.iter().filter(|&&v| v > 0).count();
        let closed_edges = closed.iter().filter(|&&v| v > 0).count();

        log::info!(
            "[Edge::Morphology] - Close done: src_edges={}, closed_edges={}, \
             net_change={:+}. Elapsed: {}ms",
            src_edges,
            closed_edges,
            closed_edges as i64 - src_edges as i64,
            start.elapsed().as_millis()
        );

        closed
    }
}
