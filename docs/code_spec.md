# DocQuad 项目代码约束规范 (v1.0)

## 1. 日志规范 (Logging)

项目统一使用 Rust 官方 `log` crate 作为日志抽象。

### 1.1 语言与风格

- **日志语言**：所有日志内容必须使用 **英文**。
- **级别定义**：
  - `error!`: 严重逻辑故障、内存映射失败、硬件不支持的指令集。
  - `warn!`: 边界情况（如检测到四边形但凸性校验未通过）、步长（Stride）异常修正。
  - `info!`: 核心阶段的完成情况（如算法初始化成功）。
  - `debug!`: 算法中间状态（如轮廓数量、阈值计算结果）。
  - `trace!`: 极高频信息（如像素处理偏移量）。

### 1.2 格式化约束

日志必须包含上下文前缀，推荐格式：`[Module::Function] - Action: Details`。

> 示例：`debug!("[Edge::Canny] - Thresholds calculated: low={}, high={}", low, high);`

### 1.3 耗时监控 (Timing Logs)

关键算法模块必须包含耗时统计。为了保持零拷贝的高性能，严禁在生产环境下进行昂贵的字符串拼接。

- **强制统计点**：Canny 检测、Suzuki-Abe 轮廓追踪、Douglas-Peucker 拟合。
- **格式要求**：使用 `Elapsed: {}ms` 或 `{}µs`。
  > 示例：`info!("[Topology::SuzukiAbe] - Processed {} contours. Elapsed: {}ms", count, duration.as_millis());`

---

## 2. 注释与文档规范 (Comments & Docs)

### 2.1 语言要求

- **代码注释**：所有内部代码逻辑注释必须使用 **中文**。
- **文档注释**：针对 `pub` 接口的 API 文档 (///) 必须使用 **中文**（方便国内开发者集成），并在必要处保留英文术语（如 Stride, Homography）。

### 2.2 注释范式

- **复杂算法逻辑**：必须在函数内部详细解释数学原理或状态机转换逻辑。
- **TODO 处理**：必须标注具体优化方向。
  - `// TODO(perf): 考虑在此处引入多线程并行处理。`

---

## 3. 错误处理约束 (Error Handling)

- **库级错误**：统一使用 `thiserror` crate 定义 `DocQuadError`。
- **返回原则**：严禁在 `lib` 核心路径使用 `panic!`、`unwrap()` 或 `expect()`。所有潜在故障点必须通过 `Result` 向上抛出。
- **上下文包装**：在跨模块边界时，使用 `map_err` 提供更多上下文信息。

---

## 4. 性能与内存约束 (Performance & Memory)

### 4.1 零拷贝保障

- **内存访问**：除必要的二值化 Buffer 外，严禁对原始图像数据进行 `clone()` 或 `Vec::from()`。
- **生命周期**：严格管理 `DocBuffer<'a>` 的生命周期，确保 Rust 编译器能在编译期拦截任何内存越界风险。

### 4.2 内存分配控制

- **预分配 (Pre-allocation)**：针对视频流处理，中间缓冲区（如边缘检测位图）应由调用者管理或在算法初始化时预分配，严禁在 `process_frame` 循环内部进行堆分配。

---

## 5. 编码风格 (Coding Standards)

- **Edition**: 强制使用 **Rust 2024 Edition**。
- **代码质量**：提交前必须通过 `cargo clippy` 检查，不允许存在 `clippy::all` 级别的警告。
- **格式化**：统一使用 `cargo fmt`。
- **变量命名**：
  - 矩阵与向量运算遵循数学习惯（如 `mat_h` 代表单应性矩阵）。
  - 图像尺寸使用 `width`, `height`, `stride`（严禁简写为 w, h, s）。

---

## 6. 模块隔离约束 (Modular Isolation)

- **跨平台隔离**：`src/core` 严禁包含任何 `target_os = "android"` 的代码。
- **外部库依赖**：
  - `glam` 仅限用于 `src/geom` 和 `src/transform`。
  - `geo` 相关类型仅限用于轮廓生成后的几何处理阶段。
  - `bytemuck` 仅限于 `src/core/buffer` 的初始转换阶段。
