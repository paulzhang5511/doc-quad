// src/core/buffer.rs
use crate::error::DocQuadError;
use ndarray::ArrayView2;
use std::time::Instant;

/// 核心图像缓冲区，支持非连续内存 (Row Stride)。
///
/// # 生命周期
/// `'a` 绑定到原始数据切片，确保编译期内存安全。
///
/// # 字段说明
/// - `data`：原始 Y 通道字节切片引用
/// - `width`：图像有效像素宽度
/// - `height`：图像高度
/// - `stride`：每行实际字节数（含 Padding，stride >= width）
pub struct DocBuffer<'a> {
    /// 原始 Y 通道数据引用
    pub data: &'a [u8],
    /// 图像宽度（像素）
    pub width: u32,
    /// 图像高度（像素）
    pub height: u32,
    /// 行步长（字节数，含 Padding）
    pub stride: u32,
}

impl<'a> DocBuffer<'a> {
    /// 创建新的缓冲区实例，执行边界合法性校验。
    ///
    /// # 错误
    /// - `stride < width`：步长小于宽度，布局非法
    /// - `height * stride > data.len()`：数据长度不足
    /// - 乘法溢出（32 位平台超大分辨率）
    pub fn new(data: &'a [u8], width: u32, height: u32, stride: u32) -> Result<Self, DocQuadError> {
        let start = Instant::now();

        // P1 修复：使用 checked_mul 防止 32 位平台整数溢出
        let required = (height as usize)
            .checked_mul(stride as usize)
            .ok_or(DocQuadError::InvalidBuffer)?;

        if stride < width || required > data.len() {
            log::error!(
                "[Core::DocBuffer] - Buffer dimensions mismatch. data_len={}, required={}",
                data.len(),
                required
            );
            return Err(DocQuadError::InvalidBuffer);
        }

        let buffer = Self {
            data,
            width,
            height,
            stride,
        };

        log::debug!(
            "[Core::DocBuffer] - Buffer initialized: {}x{}, stride={}. Elapsed: {}µs",
            width,
            height,
            stride,
            start.elapsed().as_micros()
        );
        Ok(buffer)
    }

    /// 将缓冲区转换为 ndarray 二维视图，支持零拷贝处理 Row Stride。
    ///
    /// # 注意
    /// `from_shape_ptr` 为 unsafe 函数，直接返回 `ArrayView2`（非 Result），
    /// 因此先通过 `ndarray::Ix2` + `strides` 构造合法 shape，再进入 unsafe 块。
    /// 若将来扩展至其他像素格式（如 u16 深度图），需将 stride 除以 sizeof(T)。
    pub fn as_array_view(&self) -> Result<ArrayView2<'_, u8>, DocQuadError> {
        let rows = self.height as usize;
        let cols = self.width as usize;
        let row_stride = self.stride as usize;

        // 构造带 Stride 的 shape（行步长以元素为单位，列步长为 1）
        let shape = ndarray::Ix2(rows, cols);
        let strides = ndarray::Ix2(row_stride, 1);

        // SAFETY:
        // - data 的生命周期由 'a 保证，不会在视图存活期间被释放
        // - shape 与 strides 已通过 new() 校验：stride >= width，height * stride <= data.len()
        // - 所有访问偏移 y * row_stride + x < data.len()，不会越界
        let view = unsafe {
            ArrayView2::from_shape_ptr(
                ndarray::ShapeBuilder::strides(shape, strides),
                self.data.as_ptr(),
            )
        };

        log::debug!(
            "[Core::DocBuffer] - ArrayView2 mapped: {}x{}, row_stride={}",
            cols,
            rows,
            row_stride
        );

        Ok(view)
    }
}
