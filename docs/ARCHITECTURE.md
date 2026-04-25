# ARCHITECTURE.md

# DocQuad 架构说明

## 1. 内存模型 (Zero-Copy)
利用 `ndarray::ArrayView2` 配合 `row_stride` 直接映射外部内存。
偏移公式: `index = y * stride + x`

## 2. 算法流水线
1. **Edge**: `fast-canny` (SIMD) -> 产生单通道 U8 二值图。
2. **Topology**: Suzuki-Abe 拓扑追踪 -> 提取 `Vec<Coord<f32>>`。
3. **Geometry**: Douglas-Peucker 简化 -> 筛选 `Polygon` -> 锁定 4 顶点。
4. **Transform**: `glam` 排序与投影矩阵计算。

## 3. 性能约束
- 1080p 帧处理目标: < 15ms (典型设备)。
- 内存分配: 仅在 Canny 输出和轮廓点集阶段发生堆分配，核心路径重用 Buffer。