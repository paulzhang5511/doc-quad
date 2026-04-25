# doc-quad

纯 Rust 实现的高性能文档检测库。

[![Crates.io](https://img.shields.io/crates/v/doc-quad)](https://crates.io/crates/doc-quad)
[![License](https://img.shields.io/crates/l/doc-quad)](https://crates.io/crates/doc-quad)

## 功能特性

- **高性能**: 纯 Rust 实现，支持 SIMD 加速
- **文档检测**: 自动检测图像中的文档边界
- **边缘检测**: Canny 边缘检测与形态学操作
- **几何处理**: Douglas-Peucker 简化与四边形验证
- **内存高效**: 支持 stride 的零拷贝缓冲区处理

## 安装

添加到您的 `Cargo.toml`:

```toml
[dependencies]
doc-quad = "0.1.0"
```

## 使用示例

```rust
use doc_quad::{find_document, DocBuffer};

// 加载图像数据
let image_data: Vec<u8> = /* 您的灰度图像数据 */;
let width = 1920u32;
let height = 1080u32;

// 创建文档缓冲区
let buffer = DocBuffer::new(&image_data, width, height, width)?;

// 检测文档
let result = find_document(&buffer)?;

match result {
    Some(quad) => {
        println!("检测到文档，四边形顶点: {:?}", quad.points);
    }
    None => {
        println!("未检测到文档");
    }
}
```

## 依赖说明

- `bytemuck` - 零拷贝类型转换
- `ndarray` - 多维数组处理
- `fast-canny` - Canny 边缘检测
- `glam` - SIMD 加速的线性代数运算
- `geo-types` & `geo` - 几何类型与算法
- `thiserror` - 错误处理

## 示例程序

运行示例:

```bash
cargo run --example pc_demo
cargo run --example detect_image
```

## 许可证

本项目采用以下许可证之一:
- Apache License, Version 2.0
- MIT License

您可任选其一。