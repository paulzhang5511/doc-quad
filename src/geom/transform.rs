// src/geom/transform.rs
use glam::{Vec2, Mat3};

/// 顶点排序与坐标变换
pub struct Transformer;

impl Transformer {
    /// 将四个点排序为 [左上, 右上, 右下, 左下] 顺序。
    ///
    /// # 算法说明
    /// 1. 计算四点质心。
    /// 2. 基于各点相对质心的极角排序（图像坐标系 y 向下，atan2 产生顺时针顺序）。
    /// 3. 旋转数组，使 x+y 最小的点（左上角）位于索引 0。
    ///
    /// # 坐标系说明
    /// 输入为图像坐标系（y 轴向下）。atan2 在此坐标系下产生顺时针角度增长，
    /// 排序结果为顺时针顶点序列，符合 [左上, 右上, 右下, 左下] 的预期语义。
    pub fn sort_points(mut pts: [Vec2; 4]) -> [Vec2; 4] {
        // P2 修复：移除不必要的 Instant 计时（sort_points 不在强制统计点列表中）

        // 计算质心用于极角排序
        let center = (pts[0] + pts[1] + pts[2] + pts[3]) * 0.25;

        // P2 修复：将 unwrap() 替换为 unwrap_or，防止坐标含 NaN/inf 时 panic
        pts.sort_by(|a, b| {
            let angle_a = (a.y - center.y).atan2(a.x - center.x);
            let angle_b = (b.y - center.y).atan2(b.x - center.x);
            angle_a
                .partial_cmp(&angle_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 找到 x+y 最小的点（图像坐标系中最接近左上角的点）
        // P2 修复：将 unwrap() 替换为 unwrap_or，防止坐标含 NaN 时 panic
        let tl_idx = pts
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (a.x + a.y)
                    .partial_cmp(&(b.x + b.y))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);

        // 旋转数组使左上角点位于索引 0
        pts.rotate_left(tl_idx);

        log::debug!(
            "[Geom::Transform] - Points sorted: {:?}",
            pts.map(|p| (p.x, p.y))
        );

        pts
    }

    /// 计算单应性矩阵（Homography），将透视四边形映射到标准矩形。
    ///
    /// # 算法说明
    /// 标准实现需解 8×8 线性方程组 Ah=b（DLT 算法），利用 SVD 求最小二乘解。
    /// 当前为占位实现，返回单位矩阵。
    ///
    /// # 参数
    /// - `src`：源四边形顶点，顺序为 [左上, 右上, 右下, 左下]
    /// - `dst_w`、`dst_h`：目标矩形宽高
    ///
    // TODO(feat): 实现 DLT 算法求解单应性矩阵，需引入 SVD 分解。
    //             可考虑使用 nalgebra crate 或手动实现 4 点 DLT。
    pub fn get_homography(_src: [Vec2; 4], _dst_w: f32, _dst_h: f32) -> Mat3 {
        Mat3::IDENTITY
    }
}
