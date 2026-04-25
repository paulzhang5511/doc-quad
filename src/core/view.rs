// src/core/view.rs
use crate::core::buffer::DocBuffer;
use ndarray::ArrayView2;
use bytemuck::Pod;

/// 安全内存转换辅助工具
pub fn cast_and_view<T: Pod>(buffer: &DocBuffer<'_>) -> Result<ArrayView2<'_, T>, String> {
    // 转换原始数据切片类型并构建 ndarray 视图
    let casted_data = bytemuck::try_cast_slice::<u8, T>(buffer.data)
        .map_err(|e| format!("Bytemuck cast failed: {:?}", e))?;
    
    let shape = (buffer.height as usize, buffer.width as usize);
    let strides = (buffer.stride as usize / std::mem::size_of::<T>(), 1);

    ArrayView2::from_shape_ptr(shape.strides(strides), casted_data.as_ptr())
        .map_err(|e| format!("NDArray view creation failed: {:?}", e))
}