// src/topology/contour.rs
use crate::topology::chain::{find_next_pixel, reverse_dir};
use geo_types::Coord;
use std::time::Instant;

/// 轮廓提取器，实现简化版 Suzuki-Abe 外轮廓追踪算法。
pub struct ContourExtractor;

impl ContourExtractor {
    /// 从边缘二值图中提取所有闭合外轮廓。
    ///
    /// # 算法说明
    /// 采用简化版 Suzuki-Abe 算法：
    /// 1. 逐行扫描，找到未访问的边缘像素作为轮廓起点。
    /// 2. 从起点出发，沿 8-邻域顺时针追踪，直到回到起点形成闭合轮廓。
    /// 3. 追踪过的像素标记为已访问，避免重复提取。
    ///
    /// # 参数
    /// - `edges`：Canny 输出的二值图（0 或 255），长度为 width * height
    /// - `width`、`height`：图像尺寸
    ///
    /// # 返回
    /// 每个元素为一条闭合轮廓的坐标点集，首尾点相同（已显式闭合）。
    pub fn extract(edges: &[u8], width: u32, height: u32) -> Vec<Vec<Coord<f32>>> {
        let start = Instant::now();

        let w = width as usize;
        let h = height as usize;

        // 访问标记图：避免同一像素被多条轮廓重复追踪
        let mut visited = vec![false; w * h];
        let mut contours: Vec<Vec<Coord<f32>>> = Vec::new();

        // 逐行扫描寻找轮廓起点（跳过图像边界行/列）
        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                let idx = y * w + x;

                // 跳过非边缘像素和已访问像素
                if edges[idx] == 0 || visited[idx] {
                    continue;
                }

                // 以当前像素为起点追踪轮廓
                let contour = Self::trace_contour(edges, &mut visited, w, h, x, y);

                // 过滤过短的轮廓（少于 4 个顶点无法构成四边形）
                if contour.len() >= 4 {
                    contours.push(contour);
                }
            }
        }

        log::info!(
            "[Topology::ContourExtractor] - Extracted {} contours. Elapsed: {}ms",
            contours.len(),
            start.elapsed().as_millis()
        );

        contours
    }

    /// 从起点 (start_x, start_y) 出发，沿 8-邻域追踪单条闭合轮廓。
    ///
    /// 追踪终止条件：回到起点，或连续搜索无法找到下一个邻域像素。
    /// 返回的点集已显式闭合（首尾坐标相同）。
    fn trace_contour(
        edges: &[u8],
        visited: &mut Vec<bool>,
        width: usize,
        height: usize,
        start_x: usize,
        start_y: usize,
    ) -> Vec<Coord<f32>> {
        let mut contour: Vec<Coord<f32>> = Vec::new();

        let start_coord = Coord {
            x: start_x as f32,
            y: start_y as f32,
        };
        contour.push(start_coord);
        visited[start_y * width + start_x] = true;

        let mut cx = start_x;
        let mut cy = start_y;
        // 初始搜索方向从"右"（方向 0）开始
        let mut search_dir = 0usize;

        loop {
            let max_contour_len = width * height;
            if contour.len() > max_contour_len {
                log::debug!(
                    "[Topology::ContourExtractor] - Contour trace exceeded perimeter limit, aborting."
                );
                break;
            }
            match find_next_pixel(edges, width, height, cx, cy, search_dir) {
                None => {
                    // 无可追踪邻域，轮廓在此终止（孤立像素或末端）
                    break;
                }
                Some((nx, ny, found_dir)) => {
                    // 回到起点，轮廓闭合
                    if nx == start_x && ny == start_y {
                        // 显式添加起点作为终点，确保 LineString 闭合
                        contour.push(start_coord);
                        break;
                    }

                    let nidx = ny * width + nx;
                    if visited[nidx] {
                        // 遇到已访问像素（非起点），防止无限循环
                        contour.push(start_coord);
                        break;
                    }

                    // 记录新像素并标记访问
                    contour.push(Coord {
                        x: nx as f32,
                        y: ny as f32,
                    });
                    visited[nidx] = true;

                    // 更新当前位置，下一步从来向的反方向顺时针偏转 1 步开始搜索
                    cx = nx;
                    cy = ny;
                    search_dir = (reverse_dir(found_dir) + 1) % 8;
                }
            }

            // 防护：轮廓点数超过图像总像素数，说明出现异常循环
            if contour.len() > width * height {
                log::warn!(
                    "[Topology::ContourExtractor] - Contour trace exceeded image size, aborting at ({}, {}).",
                    cx,
                    cy
                );
                break;
            }
        }

        contour
    }
}
