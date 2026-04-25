## 1. 核心内存与步长测试 (Core & Stride Tests)

**目标**：验证 `DocBuffer` 对安卓 CameraX `row_stride` 的处理是否达到零拷贝且读取准确。

- **T1: 连续内存读取 (Contiguous Memory)**
  - **场景**：输入一个 `stride == width` 的灰度字节数组。
  - **验证**：算法能够正确识别所有像素，无偏移错误。
  - **Log 示例**：`info!("[Test::Core] - Starting contiguous memory test. Buffer size: {} bytes", size);`
- **T2: 带填充内存读取 (Strided Memory)**
  - **场景**：构造一个每行末尾带有 16 或 32 字节冗余填充（Padding）的数组（模拟 Android YUV 布局）。
  - **验证**：通过 `ndarray::ArrayView2` 访问特定坐标像素时，值与预期完全一致。
  - **注释**：`// 模拟安卓 CameraX 在某些分辨率下产生的行对齐填充。`
- **T3: 零拷贝安全性 (Bytemuck Safety)**
  - **场景**：传入非对齐或长度不匹配的裸指针切片。
  - **验证**：`DocBuffer` 应抛出 `DocQuadError::InvalidBuffer`，而非引发 Segment Fault。

---

## 2. 算法阶段性测试 (Pipeline Stage Tests)

**目标**：验证 `fast-canny` 到 `geo` 的每一个环节逻辑正确。

- **T4: 边缘检测一致性 (Canny Detection)**
  - **场景**：输入一张标准的白底黑方框图片。
  - **验证**：`fast-canny` 输出的二值图中，方框边缘像素点数为预期值。
  - **Log 耗时**：`debug!("[Test::Edge] - Canny extraction completed. Elapsed: {}µs", duration.as_micros());`
- **T5: 轮廓提取完整性 (Topology Tracing)**
  - **场景**：输入包含两个互不相交的四边形。
  - **验证**：Suzuki-Abe 算法应准确返回 2 个 `geo_types::LineString` 结构，且点集闭合。
  - **注释**：`// 确保状态机能够正确处理 8-邻域追踪，不漏掉任何拓扑连接点。`
- **T6: 四边形拟合与筛选 (Quad Selection)**
  - **场景**：图片包含一个大的文档框和几个小的杂质噪声点。
  - **验证**：Douglas-Peucker 拟合后，算法应忽略小面积轮廓，锁定顶点数为 4 且面积最大的四边形。

---

## 3. 几何与投影测试 (Geometry & Perspective Tests)

**目标**：验证 `glam` 的数学校验和排序逻辑。

- **T7: 顶点正向排序 (Vertex Sorting)**
  - **场景**：输入一个随机顺序的四边形顶点集。
  - **验证**：输出必须严格符合 `[左上, 右上, 右下, 左下]` 的时钟顺序。
  - **Log 示例**：`debug!("[Test::Geom] - Sorted vertices: {:?}. Elapsed: {}µs", sorted_pts, duration.as_micros());`
- **T8: 单应性矩阵验证 (Homography Matrix)**
  - **场景**：输入一个透视形变的四边形坐标。
  - **验证**：`glam` 计算出的 $3 \times 3$ 矩阵与已知标准解的误差在 $10^{-6}$ 以内。

---

## 4. 边界与异常情况测试 (Edge Cases)

- **T9: 无目标检测 (No Target Found)**
  - **场景**：输入纯噪点图或全黑图。
  - **验证**：算法返回 `Option::None`，且 `error!` 记录失败原因。
- **T10: 极大分辨率压力测试 (High-Res Stress)**
  - **场景**：输入 4K 分辨率（800万像素）的 Y 通道数据。
  - **验证**：内存占用保持平稳，耗时 log 需符合 PRD 15ms（基于下采样策略）的预期。
  - **Log 耗时**：`info!("[Test::Stress] - High-res 4K frame processed. Total Elapsed: {}ms", duration.as_millis());`

---

## 5. 测试用例组织约定

- **测试日志格式**：
  每个测试开始前输出 `[Test::Name] - Start`，结束后输出耗时总结。
- **数据准备**：
  测试图片统一存放在 `tests/fixtures/` 目录下，包含灰度位图（.pgm）和标注过的顶点坐标（.json）。
- **并行执行**：
  利用 `cargo test` 的特性，确保几何模块的测试是线程安全的。
