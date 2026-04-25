// src/edge/threshold.rs
use ndarray::ArrayView2;
use std::time::Instant;

/// 自适应阈值计算器
pub struct AdaptiveThreshold;

impl AdaptiveThreshold {
    /// 基于图像直方图统计计算 Canny 高低阈值。
    ///
    /// # 分支策略
    ///
    /// ## 极暗场景（中位数 <= 10）
    /// 大部分像素为背景黑色，中位数法失效，改用高百分位数法（p70/p85）。
    ///
    /// ## 高亮背景场景（中位数 > 150）
    /// 典型真实照片：背景为白纸/浅色桌面，中位数反映背景亮度而非边缘强度。
    /// 进一步细分为极度高亮（动态范围 < 30 或高亮区集中度 < 10）和正常高亮两个子分支。
    ///
    /// ## 正常场景（10 < 中位数 <= 150）
    /// 标准中位数法：low = (1-sigma)*median，high = (1+sigma)*median，
    /// 同时对 high 施加 200.0 的硬上限，防止边界情况产生过高阈值。
    ///
    /// sigma 推荐值为 0.33。
    pub fn calculate(view: &ArrayView2<'_, u8>, sigma: f32) -> (f32, f32) {
        let start = Instant::now();

        // 统计 256 级灰度直方图
        let mut hist = [0u32; 256];
        for &pixel in view.iter() {
            hist[pixel as usize] += 1;
        }

        let total = view.len() as u32;

        // 累积分布求中位数（第 50 百分位）
        let mut count = 0u32;
        let mut median = 127u8;
        for (i, &c) in hist.iter().enumerate() {
            count += c;
            if count >= total / 2 {
                median = i as u8;
                break;
            }
        }

        let (low, high) = if median <= 10 {
            // ── 极暗/稀疏场景：使用高百分位数法 ──────────────────────────────
            // 中位数 <= 10 说明大部分像素为背景黑色（如白纸上的黑色文字扫描件），
            // 此时中位数法失效，改用高百分位数法保证边缘能被检测到。
            // 取第 70 百分位作为 low 阈值基准，第 85 百分位作为 high 阈值基准。
            let p70 = Self::percentile(&hist, total, 70);
            let p85 = Self::percentile(&hist, total, 85);

            // 若高百分位仍为 0（全黑图），使用固定保底阈值
            let low = if p70 > 0 {
                (p70 as f32 * (1.0 - sigma)).max(1.0)
            } else {
                10.0
            };
            let high = if p85 > 0 {
                (p85 as f32).max(low + 1.0)
            } else {
                30.0
            };

            log::debug!(
                "[Edge::Threshold] - Sparse scene (median={}), percentile method: \
                 p70={}, p85={}, low={:.2}, high={:.2}",
                median, p70, p85, low, high
            );

            (low, high)
        } else if median > 150 {
            // ── 高亮背景场景：使用直方图差分近似梯度分布 ────────────────────────
            //
            // 核心问题：p75=242, p90=245 相差仅 3，说明直方图极度集中在高亮区。
            // 此时像素亮度百分位与 Sobel 梯度幅值完全脱钩，不能用来估算 Canny 阈值。
            //
            // 修复策略：
            // 1. 计算直方图的"集中度"指标：p90 - p10（亮度动态范围）
            // 2. 若动态范围 < 30（极度高亮/低对比度场景），切换到固定低阈值模式
            // 3. 否则使用改进的 p25/p75 百分位（聚焦暗区，捕捉文档边缘弱梯度）
            let p10 = Self::percentile(&hist, total, 10);
            let p25 = Self::percentile(&hist, total, 25);
            let p75 = Self::percentile(&hist, total, 75);
            let p90 = Self::percentile(&hist, total, 90);

            let dynamic_range = p90 as i32 - p10 as i32;
            // 高亮区集中度：p90 与 p75 之差越小说明高亮区越集中
            let highlight_spread = p90 as i32 - p75 as i32;

            let (low, high) = if dynamic_range < 30 || highlight_spread < 10 {
                // 极度高亮场景（白纸拍白桌）：整个图像对比度极低，
                // 文档边缘的 Sobel 梯度幅值极小，必须使用固定低阈值才能检测到。
                // 经验值：文档边缘在此类场景下梯度幅值通常在 10~40 之间。
                let low = 5.0f32;
                let high = 20.0f32;

                log::debug!(
                    "[Edge::Threshold] - Extreme highlight scene (median={}, dynamic_range={}, \
                     highlight_spread={}), using fixed low thresholds: low={:.2}, high={:.2}",
                    median, dynamic_range, highlight_spread, low, high
                );

                (low, high)
            } else {
                // 正常高亮背景（有足够对比度）：使用 p25/p75 聚焦暗区
                // p25 通常落在文档内容区（文字、印章），p75 对应边缘过渡区
                let low = (p25 as f32 * (1.0 - sigma)).max(1.0).min(60.0);
                let high = (p75 as f32 * (1.0 - sigma * 0.5))
                    .max(low + 5.0)
                    .min(150.0);

                log::debug!(
                    "[Edge::Threshold] - Bright background scene (median={}, dynamic_range={}), \
                     p25/p75 method: p25={}, p75={}, low={:.2}, high={:.2}",
                    median, dynamic_range, p25, p75, low, high
                );

                (low, high)
            };

            (low, high)
        } else {
            // ── 正常场景：标准中位数法 + 硬上限 ──────────────────────────────
            // 中位数在 10~150 之间，图像亮度分布适中，标准中位数法有效。
            // 对 high 施加 200.0 的硬上限（P0 修复），防止中位数偏高时
            // high 被截断到 255，导致与高亮背景场景相同的边缘丢失问题。
            let low = (0.0f32.max((1.0 - sigma) * median as f32)).min(255.0);
            let high = (255.0f32.min((1.0 + sigma) * median as f32))
                .max(low + 1.0)
                .min(200.0); // P0 修复：hard cap，防止 high 过高

            (low, high)
        };

        log::debug!(
            "[Edge::Threshold] - Thresholds: low={:.2}, high={:.2}, median={}, \
             sigma={:.2}. Elapsed: {}µs",
            low,
            high,
            median,
            sigma,
            start.elapsed().as_micros()
        );

        (low, high)
    }

    /// 从直方图中计算指定百分位数对应的像素值。
    ///
    /// # 参数
    /// - `hist`：256 级灰度直方图
    /// - `total`：总像素数
    /// - `percent`：百分位（0~100）
    fn percentile(hist: &[u32; 256], total: u32, percent: u32) -> u8 {
        // 目标累积像素数
        let target = (total as u64 * percent as u64 / 100) as u32;
        let mut cumulative = 0u32;
        for (i, &c) in hist.iter().enumerate() {
            cumulative += c;
            if cumulative >= target {
                return i as u8;
            }
        }
        255
    }

    /// 辅助方法：快速计算近似中值（用于低功耗场景）。
    ///
    /// 以步长 4 采样，牺牲精度换取速度，适用于实时预览帧。
    pub fn fast_median(view: &ndarray::ArrayView2<'_, u8>) -> u8 {
        let sample_step = 4;
        let mut hist = [0u32; 256];
        let mut count = 0u32;

        for row in view.rows().into_iter().step_by(sample_step) {
            for &pixel in row.iter().step_by(sample_step) {
                hist[pixel as usize] += 1;
                count += 1;
            }
        }

        if count == 0 {
            return 127;
        }

        let mut cumulative = 0u32;
        for (i, &c) in hist.iter().enumerate() {
            cumulative += c;
            if cumulative >= count / 2 {
                return i as u8;
            }
        }
        127
    }
}
