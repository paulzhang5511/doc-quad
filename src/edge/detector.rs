// src/edge/detector.rs
use crate::core::buffer::DocBuffer;
use crate::edge::morphology::Morphology;
use crate::edge::threshold::AdaptiveThreshold;
use crate::error::DocQuadError;
use fast_canny::{CannyConfig, CannyWorkspace, canny_u8};
use std::time::Instant;

/// 边缘检测器封装，持有预分配的 CannyWorkspace 以避免帧间堆分配。
pub struct EdgeDetector {
    workspace: CannyWorkspace,
    width: usize,
    height: usize,
}

impl EdgeDetector {
    pub fn new(width: usize, height: usize) -> Result<Self, DocQuadError> {
        let workspace = CannyWorkspace::new(width, height).map_err(|e| {
            log::error!(
                "[Edge::Detector] - Failed to create CannyWorkspace: {:?}",
                e
            );
            DocQuadError::EdgeDetectionError
        })?;

        log::info!(
            "[Edge::Detector] - CannyWorkspace pre-allocated for {}x{}.",
            width,
            height
        );

        Ok(Self {
            workspace,
            width,
            height,
        })
    }

    pub fn detect(&mut self, buffer: &DocBuffer<'_>) -> Result<Vec<u8>, DocQuadError> {
        self.detect_with_debug(buffer, None)
    }

    /// 执行 Canny 边缘检测，可选输出调试中间图到指定目录。
    ///
    /// # 参数
    /// - `buffer`：输入图像缓冲区
    /// - `debug_dir`：若 Some(path)，则将各阶段中间图保存到该目录
    pub fn detect_with_debug(
        &mut self,
        buffer: &DocBuffer<'_>,
        debug_dir: Option<&std::path::Path>,
    ) -> Result<Vec<u8>, DocQuadError> {
        let start = Instant::now();

        let view = buffer.as_array_view()?;

        // ── 阈值计算 ──────────────────────────────────────────────────────────
        let (low, high) = AdaptiveThreshold::calculate(&view, 0.33);

        // 输出详细的直方图统计，辅助判断阈值是否合理
        let mut hist = [0u32; 256];
        for &pixel in view.iter() {
            hist[pixel as usize] += 1;
        }
        let total_pixels = (buffer.width * buffer.height) as f32;

        // 统计各亮度区间的像素占比，辅助诊断图像亮度分布
        let dark_pct = hist[0..64].iter().sum::<u32>() as f32 / total_pixels * 100.0;
        let mid_pct = hist[64..192].iter().sum::<u32>() as f32 / total_pixels * 100.0;
        let bright_pct = hist[192..256].iter().sum::<u32>() as f32 / total_pixels * 100.0;

        log::info!(
            "[Edge::Detector] - Pixel brightness distribution: \
             dark(0-63)={:.1}%, mid(64-191)={:.1}%, bright(192-255)={:.1}%",
            dark_pct, mid_pct, bright_pct
        );

        // 输出直方图峰值区间（找出像素最集中的 16 级区间）
        let mut max_bucket_count = 0u32;
        let mut max_bucket_start = 0usize;
        for i in (0..256).step_by(16) {
            let bucket_sum: u32 = hist[i..i + 16].iter().sum();
            if bucket_sum > max_bucket_count {
                max_bucket_count = bucket_sum;
                max_bucket_start = i;
            }
        }
        log::info!(
            "[Edge::Detector] - Histogram peak bucket: [{}-{}] = {} pixels ({:.1}%)",
            max_bucket_start,
            max_bucket_start + 15,
            max_bucket_count,
            max_bucket_count as f32 / total_pixels * 100.0
        );

        log::info!(
            "[Edge::Detector] - Adaptive thresholds: low={:.2}, high={:.2} \
             (image={}x{}, sigma=0.33)",
            low, high, buffer.width, buffer.height
        );

        if high > 200.0 {
            log::warn!(
                "[Edge::Detector] - Canny high threshold {:.2} is very high (>200). \
                 Document edges with weak gradients may be missed.",
                high
            );
        }

        // ── 内存紧凑化 ────────────────────────────────────────────────────────
        let input_data: Vec<u8> = if buffer.stride == buffer.width {
            log::debug!(
                "[Edge::Detector] - Input is contiguous (stride==width={}), direct copy.",
                buffer.width
            );
            buffer.data[..(buffer.width * buffer.height) as usize].to_vec()
        } else {
            log::debug!(
                "[Edge::Detector] - Input has stride padding (stride={} > width={}), compacting rows.",
                buffer.stride, buffer.width
            );
            let mut compact = Vec::with_capacity((buffer.width * buffer.height) as usize);
            for row in view.rows() {
                compact.extend(row.iter().copied());
            }
            compact
        };

        // 保存输入灰度图（调试用）
        if let Some(dir) = debug_dir {
            Self::save_gray_image(
                &input_data,
                self.width,
                self.height,
                &dir.join("debug_01_input_gray.png"),
            );
        }

        // ── Canny 检测 ────────────────────────────────────────────────────────
        // 尝试三组阈值，输出各自的边缘密度，辅助判断最佳阈值区间
        log::info!(
            "[Edge::Detector] - [DEBUG] Threshold sensitivity scan \
             (sigma=1.0, 3 trial configs):"
        );
        for &(trial_low, trial_high) in &[(2.0f32, 8.0), (5.0, 20.0), (10.0, 40.0)] {
            if let Ok(cfg) = CannyConfig::builder()
                .sigma(1.0)
                .thresholds(trial_low, trial_high)
                .build()
            {
                // 注意：此处复用 workspace，trial 结果会覆盖，仅用于统计密度
                if let Ok(trial_slice) = canny_u8(&input_data, &mut self.workspace, &cfg) {
                    let trial_count = trial_slice.iter().filter(|&&v| v == 255).count();
                    let trial_density =
                        trial_count as f32 / (self.width * self.height) as f32 * 100.0;
                    log::info!(
                        "[Edge::Detector] - [DEBUG]   low={:.1}, high={:.1} -> \
                         edge_pixels={}, density={:.2}%",
                        trial_low, trial_high, trial_count, trial_density
                    );
                }
            }
        }

        // 同时尝试 sigma=0.5 的低平滑版本，观察细边缘保留情况
        log::info!(
            "[Edge::Detector] - [DEBUG] Low-sigma scan (sigma=0.5, low=5.0, high=20.0):"
        );
        if let Ok(cfg_low_sigma) = CannyConfig::builder()
            .sigma(0.5)
            .thresholds(5.0, 20.0)
            .build()
        {
            if let Ok(trial_slice) = canny_u8(&input_data, &mut self.workspace, &cfg_low_sigma) {
                let trial_count = trial_slice.iter().filter(|&&v| v == 255).count();
                let trial_density =
                    trial_count as f32 / (self.width * self.height) as f32 * 100.0;
                log::info!(
                    "[Edge::Detector] - [DEBUG]   sigma=0.5, low=5.0, high=20.0 -> \
                     edge_pixels={}, density={:.2}%",
                    trial_count, trial_density
                );
            }
        }

        // 正式 Canny 检测（使用自适应阈值）
        let cfg = CannyConfig::builder()
            .sigma(1.0)
            .thresholds(low, high)
            .build()
            .map_err(|e| {
                log::error!("[Edge::Detector] - Invalid Canny config: {:?}", e);
                DocQuadError::EdgeDetectionError
            })?;

        let canny_start = Instant::now();
        let edge_slice = canny_u8(&input_data, &mut self.workspace, &cfg).map_err(|e| {
            log::error!("[Edge::Detector] - canny_u8 failed: {:?}", e);
            DocQuadError::EdgeDetectionError
        })?;

        let raw_edges = edge_slice.to_vec();
        let canny_elapsed = canny_start.elapsed().as_millis();
        let raw_edge_count = raw_edges.iter().filter(|&&v| v == 255).count();
        let raw_density = raw_edge_count as f32 / (self.width * self.height) as f32 * 100.0;

        log::info!(
            "[Edge::Detector] - Canny raw result: edge_pixels={}, density={:.2}%, \
             size={}x{}. Canny elapsed: {}ms",
            raw_edge_count, raw_density, self.width, self.height, canny_elapsed
        );

        // 分析边缘像素的空间分布（分 4×4 网格统计密度）
        Self::log_edge_spatial_distribution(&raw_edges, self.width, self.height, "raw_canny");

        // 保存 Canny 原始边缘图
        if let Some(dir) = debug_dir {
            Self::save_binary_image(
                &raw_edges,
                self.width,
                self.height,
                &dir.join("debug_02_canny_raw.png"),
            );
        }

        // ── 形态学闭运算 ──────────────────────────────────────────────────────
        let morph_radius = Self::choose_morph_radius(self.width, self.height);
        log::debug!(
            "[Edge::Detector] - Applying morphological close: radius={} (image={}x{})",
            morph_radius, self.width, self.height
        );

        let closed_edges = Morphology::close(&raw_edges, self.width, self.height, morph_radius);

        let closed_edge_count = closed_edges.iter().filter(|&&v| v == 255).count();
        let closed_density =
            closed_edge_count as f32 / (self.width * self.height) as f32 * 100.0;
        let net_change = closed_edge_count as i64 - raw_edge_count as i64;
        let growth_pct = if raw_edge_count > 0 {
            net_change as f32 / raw_edge_count as f32 * 100.0
        } else {
            0.0
        };

        log::info!(
            "[Edge::Detector] - After morphological close: edge_pixels={}, density={:.2}%, \
             net_change={:+} ({:+.1}%). Total elapsed: {}ms",
            closed_edge_count, closed_density, net_change, growth_pct,
            start.elapsed().as_millis()
        );

        // 分析闭运算后边缘的空间分布
        Self::log_edge_spatial_distribution(
            &closed_edges,
            self.width,
            self.height,
            "after_close",
        );

        // 保存闭运算后边缘图
        if let Some(dir) = debug_dir {
            Self::save_binary_image(
                &closed_edges,
                self.width,
                self.height,
                &dir.join("debug_03_after_close.png"),
            );

            // 额外：用 radius=2 再做一次闭运算，对比效果
            let closed_r2 = Morphology::close(&raw_edges, self.width, self.height, 2);
            let r2_count = closed_r2.iter().filter(|&&v| v == 255).count();
            log::info!(
                "[Edge::Detector] - [DEBUG] radius=2 close result: \
                 edge_pixels={}, density={:.2}%",
                r2_count,
                r2_count as f32 / (self.width * self.height) as f32 * 100.0
            );
            Self::save_binary_image(
                &closed_r2,
                self.width,
                self.height,
                &dir.join("debug_04_close_radius2.png"),
            );

            // 额外：用 radius=3 再做一次闭运算
            let closed_r3 = Morphology::close(&raw_edges, self.width, self.height, 3);
            let r3_count = closed_r3.iter().filter(|&&v| v == 255).count();
            log::info!(
                "[Edge::Detector] - [DEBUG] radius=3 close result: \
                 edge_pixels={}, density={:.2}%",
                r3_count,
                r3_count as f32 / (self.width * self.height) as f32 * 100.0
            );
            Self::save_binary_image(
                &closed_r3,
                self.width,
                self.height,
                &dir.join("debug_05_close_radius3.png"),
            );

            // 额外：sigma=0.5 + radius=2 组合
            if let Ok(cfg_s05) = CannyConfig::builder()
                .sigma(0.5)
                .thresholds(5.0, 20.0)
                .build()
            {
                if let Ok(s05_slice) = canny_u8(&input_data, &mut self.workspace, &cfg_s05) {
                    let s05_raw = s05_slice.to_vec();
                    let s05_closed = Morphology::close(&s05_raw, self.width, self.height, 2);
                    let s05_count = s05_closed.iter().filter(|&&v| v == 255).count();
                    log::info!(
                        "[Edge::Detector] - [DEBUG] sigma=0.5 + radius=2 close: \
                         edge_pixels={}, density={:.2}%",
                        s05_count,
                        s05_count as f32 / (self.width * self.height) as f32 * 100.0
                    );
                    Self::save_binary_image(
                        &s05_closed,
                        self.width,
                        self.height,
                        &dir.join("debug_06_sigma05_close_r2.png"),
                    );
                }
            }
        }

        if closed_density > 25.0 {
            log::warn!(
                "[Edge::Detector] - Post-close edge density {:.2}% is very high (>25%). \
                 Morphological close may have over-connected noise. \
                 Consider reducing morph_radius ({}) or raising Canny thresholds.",
                closed_density, morph_radius
            );
        } else if closed_density < 0.05 {
            log::warn!(
                "[Edge::Detector] - Post-close edge density {:.2}% is very low (<0.05%). \
                 Document edges may be missing. \
                 Consider lowering Canny thresholds (current low={:.2}, high={:.2}).",
                closed_density, low, high
            );
        }

        if growth_pct > 100.0 {
            log::warn!(
                "[Edge::Detector] - Morphological close growth rate {:.1}% is very high (>100%). \
                 radius={} may be too large, causing noise over-connection.",
                growth_pct, morph_radius
            );
        }

        Ok(closed_edges)
    }

    /// 将边缘图按 4×4 网格分区，统计每个分区的边缘密度，辅助判断边缘的空间分布。
    ///
    /// 若文档边框存在，应在图像四周（边缘区域）出现高密度；
    /// 若只有内容噪声，则密度集中在图像中央。
    fn log_edge_spatial_distribution(
        edges: &[u8],
        width: usize,
        height: usize,
        label: &str,
    ) {
        let grid_cols = 4usize;
        let grid_rows = 4usize;
        let cell_w = width / grid_cols;
        let cell_h = height / grid_rows;

        log::info!(
            "[Edge::Detector] - [DEBUG] Edge spatial distribution ({}) \
             in {}x{} grid (cell={}x{}px):",
            label, grid_cols, grid_rows, cell_w, cell_h
        );

        let cell_total = (cell_w * cell_h) as f32;
        let mut grid_lines = Vec::new();

        for gy in 0..grid_rows {
            let mut row_str = String::from("  |");
            for gx in 0..grid_cols {
                let x0 = gx * cell_w;
                let y0 = gy * cell_h;
                let x1 = ((gx + 1) * cell_w).min(width);
                let y1 = ((gy + 1) * cell_h).min(height);

                let mut count = 0usize;
                for y in y0..y1 {
                    for x in x0..x1 {
                        if edges[y * width + x] == 255 {
                            count += 1;
                        }
                    }
                }
                let density = count as f32 / cell_total * 100.0;
                // 用简单字符可视化密度：空格<1%, .=1-5%, o=5-15%, O=15-30%, #>30%
                let symbol = if density < 1.0 {
                    " "
                } else if density < 5.0 {
                    "."
                } else if density < 15.0 {
                    "o"
                } else if density < 30.0 {
                    "O"
                } else {
                    "#"
                };
                row_str.push_str(&format!("{:>5.1}%{}|", density, symbol));
            }
            grid_lines.push(row_str);
        }

        // 输出分隔线
        let separator = format!("  +{}+", "------+".repeat(grid_cols));
        log::info!("[Edge::Detector] - [DEBUG] {}", separator);
        for line in &grid_lines {
            log::info!("[Edge::Detector] - [DEBUG] {}", line);
        }
        log::info!("[Edge::Detector] - [DEBUG] {}", separator);
    }

    /// 将灰度字节数组保存为 PNG 图像（调试用，仅在 debug_dir 存在时调用）。
    fn save_gray_image(data: &[u8], width: usize, height: usize, path: &std::path::Path) {
        // 使用标准库写入 PGM 格式（无需 image crate，避免循环依赖）
        // PGM 是最简单的灰度图格式，可用 GIMP/Photoshop/ImageMagick 打开
        match Self::write_pgm(data, width, height, path.with_extension("pgm").as_path()) {
            Ok(_) => log::info!(
                "[Edge::Detector] - [DEBUG] Saved gray image: {}",
                path.with_extension("pgm").display()
            ),
            Err(e) => log::warn!(
                "[Edge::Detector] - [DEBUG] Failed to save gray image {}: {}",
                path.display(),
                e
            ),
        }
    }

    /// 将二值边缘图保存为 PGM 图像（255=白色边缘，0=黑色背景）。
    fn save_binary_image(data: &[u8], width: usize, height: usize, path: &std::path::Path) {
        match Self::write_pgm(data, width, height, path.with_extension("pgm").as_path()) {
            Ok(_) => log::info!(
                "[Edge::Detector] - [DEBUG] Saved binary image: {}",
                path.with_extension("pgm").display()
            ),
            Err(e) => log::warn!(
                "[Edge::Detector] - [DEBUG] Failed to save binary image {}: {}",
                path.display(),
                e
            ),
        }
    }

    /// 写入 PGM（Portable GrayMap）文件，格式简单，无需外部依赖。
    fn write_pgm(
        data: &[u8],
        width: usize,
        height: usize,
        path: &std::path::Path,
    ) -> std::io::Result<()> {
        use std::io::Write;

        // 确保父目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = std::fs::File::create(path)?;
        // PGM 头部：P5 = 二进制灰度图，最大值 255
        write!(file, "P5\n{} {}\n255\n", width, height)?;
        file.write_all(&data[..width * height])?;
        Ok(())
    }

fn choose_morph_radius(width: usize, height: usize) -> usize {
    let long_edge = width.max(height);
    if long_edge <= 512 { 1 }
    else if long_edge <= 1024 { 2 }  // 当前固定返回 1，建议改为 2
    else { 3 }
}
}
