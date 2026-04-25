// src/geom/mod.rs
pub mod simplify;
pub mod validate;
pub mod transform;

use glam::Vec2;

/// 代表检测到的文档四边形。
///
/// 顶点顺序固定为 [左上, 右上, 右下, 左下]（图像坐标系，y 向下）。
#[derive(Debug, Clone)]
pub struct Quadrilateral {
    /// 顶点数组，顺序：[左上, 右上, 右下, 左下]
    pub points: [Vec2; 4],
    /// 四边形围成的面积（像素²）
    pub area: f32,
    /// 置信度评分（0.0 ~ 1.0）
    pub score: f32,
}
