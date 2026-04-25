// src/core/mod.rs
pub mod buffer;
// P2 修复：view.rs 为死代码（cast_and_view 从未被调用，且违反 bytemuck 使用约束），已删除
