// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DocQuadError {
    #[error("Invalid buffer dimensions or alignment")]
    InvalidBuffer,

    #[error("Memory mapping failed: {0}")]
    MappingError(String),

    #[error("Edge detection failed")]
    EdgeDetectionError,

    #[error("No valid document quadrilateral found")]
    NoQuadFound,

    #[error("Internal geometry error")]
    GeometryError,

    // P2 修复：StrideMismatch 现在在 topology 模块中实际使用
    #[error("Inconsistent row stride: {0}")]
    StrideMismatch(String),
}
