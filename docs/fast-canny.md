# fast-canny

**工业级零分配 SIMD Canny 边缘检测器**

基于 Rust 实现的高性能 Canny 边缘检测库，专为实时图像处理场景设计。
通过 AVX2+FMA / NEON 硬件加速、零堆分配 BFS 和 L1 Cache 友好的 Tiling
流水线，在保持算法精度的同时实现极致吞吐量。

---

## 目录

- [项目介绍](#项目介绍)
- [功能特性](#功能特性)
- [技术架构](#技术架构)
- [安装与配置](#安装与配置)
- [快速上手](#快速上手)
- [API 文档](#api-文档)
- [示例程序](#示例程序)
- [测试与验证](#测试与验证)
- [性能基准](#性能基准)

---

## 项目介绍

Canny 边缘检测是计算机视觉领域最经典的算法之一，广泛应用于：

- 工业质检（缺陷轮廓提取）
- 自动驾驶（车道线、障碍物检测）
- 医学影像（病灶边界分割）
- 实时视频流处理

`fast-canny` 针对以下痛点进行了系统性优化：

| 痛点              | 解决方案                         |
| ----------------- | -------------------------------- |
| 每帧大量堆分配    | Bump Arena + Zero-Copy Workspace |
| 单线程瓶颈        | Rayon 多核 Tiling 流水线         |
| 标量 Sobel 计算慢 | AVX2+FMA / NEON SIMD 内核        |
| 硬件耦合难移植    | `SobelKernel` trait 运行时分发   |

---

## 功能特性

### 算法流水线（五阶段）

```
输入 f32 灰度图
    │
    ▼
① 高斯平滑      — 基于 libblur，可配置 sigma，sigma≤0 时跳过
    │
    ▼
② Sobel 梯度    — 融合幅值+方向，AVX2 一次处理 8 像素
    │
    ▼
③ NMS           — 非极大值抑制，细化为单像素宽边缘
    │
    ▼
④ 双阈值化      — 强边缘(255) / 弱边缘(127) / 非边缘(0)
    │
    ▼
⑤ Hysteresis    — 零分配 BFS，弱边缘连通强边缘则提拔，否则丢弃
    │
    ▼
输出 u8 二值边缘图（仅含 0 / 255）
```

### 核心特性

- **零帧间分配**：`CannyWorkspace` 预分配所有缓冲区，帧间仅执行 `O(W×H)` 清零
- **SIMD 加速**：x86_64 优先启用 AVX2+FMA，aarch64 使用 NEON，自动回退标量
- **多核 Tiling**：`TILE_SIZE=64` 的 L1 Cache 友好分块，Rayon 并行处理行 Tile
- **独立方向缓冲**：`dir_map` 与 `edge_map` 分离，消除原地覆盖 UB
- **边界安全内聚**：`track_edges` 入口主动清零四周边界，不依赖调用方约定
- **C-FFI 支持**：提供 `canny_workspace_new` / `canny_process_ex` 等 C 接口

---

## 技术架构

### 模块结构

```
src/
├── lib.rs              # 对外 API：canny()、CannyConfig、CannyError、C-FFI
├── workspace.rs        # CannyWorkspace：预分配缓冲区 + Bump Arena
├── gaussian.rs         # 高斯平滑（委托 libblur）
├── kernel/
│   ├── mod.rs          # SobelKernel trait + 运行时分发 detect()
│   ├── avx2.rs         # AVX2+FMA Sobel 融合算子
│   └── aarch64.rs      # NEON Sobel 融合算子（aarch64）
├── pipeline.rs         # Tiling 多核流水线 execute_tiled_pipeline()
├── nms.rs              # NMS + 双阈值化融合算子
├── hysteresis.rs       # 零分配 BFS Hysteresis
└── pipeline_ptr.rs     # SendPtr / SendConstPtr 跨线程指针包装
```

### 内存布局（CannyWorkspace）

```
buffer_b  [f32 × W×H]  ← 高斯平滑输出（Sobel 输入）
buffer_a  [f32 × W×H]  ← 梯度幅值 (mag)
dir_map   [u8  × W×H]  ← 梯度方向 (0/1/2/3)
edge_map  [u8  × W×H]  ← NMS 输出 / Hysteresis 最终结果
arena     [Bump]        ← BFS 栈（帧间 O(1) reset）
kernel    [Box&lt;dyn SobelKernel&gt;]  ← 运行时选定的硬件实现
```

### 依赖说明

| 依赖       | 版本   | 用途                                          |
| ---------- | ------ | --------------------------------------------- |
| `rayon`    | 1.10   | 多核并行 Tiling                               |
| `bumpalo`  | 3.16   | 零分配 Bump Arena（需 `collections` feature） |
| `bytemuck` | 1.15   | 安全类型转换                                  |
| `log`      | 0.4    | 结构化日志（trace/debug/info/warn/error）     |
| `libblur`  | 0.23.3 | 高性能高斯模糊（支持 f32 单通道）             |

开发/测试依赖：

| 依赖         | 用途             |
| ------------ | ---------------- |
| `image`      | 示例程序图像 I/O |
| `env_logger` | 测试日志输出     |

---

## 安装与配置

### 环境要求

- Rust 2024 Edition（`edition = "2024"`）
- Rust 工具链 ≥ 1.80（稳定版）
- x86_64：支持 AVX2+FMA 的 CPU（Haswell 及以后）
- aarch64：ARMv8-A 及以后（NEON 强制基线）

### 作为库依赖引入

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
fast-canny = { path = "path/to/fast-canny" }
```

### 启用 AVX2 编译优化

在项目根目录创建 `.cargo/config.toml`：

```toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "target-feature=+avx2,+fma"]

[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+avx2,+fma"]
```

> **注意**：不设置此项时，库会在运行时通过 `is_x86_feature_detected!` 自动检测
> 并回退到标量实现，无需手动配置也能正确运行。

### 构建

```bash
# Debug 构建
cargo build

# Release 构建（启用 LTO + 全优化）
cargo build --release

# 构建动态库（cdylib，供 C/Android 调用）
cargo build --release --lib
```

---

## 快速上手

### f32 输入（主 API）

```rust
use fast_canny::{CannyConfig, CannyWorkspace, canny};

fn main() {
    // 1. 创建 Workspace（一次性，可跨帧复用）
    let (width, height) = (1920usize, 1080usize);
    let mut ws = CannyWorkspace::new(width, height)
        .expect("尺寸必须 >= 3x3");

    // 2. 配置参数
    let cfg = CannyConfig::builder()
        .sigma(1.0)               // 高斯平滑强度，0.0 跳过
        .thresholds(50.0, 150.0)  // 低阈值 / 高阈值
        .build()
        .expect("low 必须 <= high");

    // 3. 准备输入（f32 灰度图，值域建议 [0, 255]）
    let src: Vec&lt;f32&gt; = vec![0.0f32; width * height];

    // 4. 执行检测（返回 &[u8]，生命周期绑定到 ws）
    let edge_map: &[u8] = canny(&src, &mut ws, &cfg)
        .expect("输入长度必须等于 width * height");

    // 5. edge_map 仅含 0（非边缘）和 255（边缘）
    let edge_count = edge_map.iter().filter(|&&v| v == 255).count();
    println!("检测到 {} 个边缘像素", edge_count);

    // 6. 下一帧：直接复用 ws，无需重新分配
    let next_frame: Vec&lt;f32&gt; = vec![128.0f32; width * height];
    let _ = canny(&next_frame, &mut ws, &cfg).unwrap();
}
```

### u8 输入（`canny_u8` API）

适用于直接来自摄像头或图像解码的 u8 灰度图，内部自动完成 u8→f32 转换并写入预分配缓冲区，无额外堆分配：

```rust
use fast_canny::{CannyConfig, CannyWorkspace, canny_u8};

fn main() {
    let (width, height) = (640usize, 480usize);
    let mut ws = CannyWorkspace::new(width, height).unwrap();

    let cfg = CannyConfig::builder()
        .sigma(1.0)
        .thresholds(30.0, 90.0)
        .build()
        .unwrap();

    // u8 灰度图，每像素 1 字节
    let src: Vec&lt;u8&gt; = vec![128u8; width * height];

    let edge_map = canny_u8(&src, &mut ws, &cfg).unwrap();
    println!("边缘像素数: {}", edge_map.iter().filter(|&&v| v == 255).count());
}
```

### 使用 Builder 配置

```rust
use fast_canny::CannyConfig;

// 标准配置
let cfg = CannyConfig::builder()
    .sigma(1.4)
    .thresholds(30.0, 90.0)
    .build()?;

// 跳过高斯平滑（适合已预处理的输入）
let cfg_no_blur = CannyConfig::builder()
    .sigma(0.0)
    .thresholds(10.0, 30.0)
    .build()?;

// 使用默认值（sigma=1.0, low=50.0, high=150.0）
let cfg_default = CannyConfig::default();
```

### C-FFI 调用示例

```c
#include <stdint.h>
#include <stddef.h>

void* canny_workspace_new(size_t width, size_t height);
void  canny_workspace_free(void* ws);
int   canny_process_ex(
    void* ws,
    const float* src, size_t src_len,
    float sigma, float low_thresh, float high_thresh,
    const uint8_t** out_edge
);

int main() {
    void* ws = canny_workspace_new(640, 480);
    if (!ws) return -1;

    float src[640 * 480] = {0};
    const uint8_t* edge = NULL;

    int status = canny_process_ex(
        ws, src, 640 * 480,
        1.0f, 50.0f, 150.0f,
        &edge
    );

    if (status == 0) {
        // 使用 edge 指针（生命周期与 ws 相同）
    }

    canny_workspace_free(ws);
    return 0;
}
```

---

## API 文档

### `CannyWorkspace`

```rust
// 创建（尺寸必须 >= 3x3，否则返回 Err(InvalidDimensions)）
pub fn new(width: usize, height: usize) -> Result<Self, CannyError>

// 帧间重置：O(W×H) 清零 edge_map，O(1) 重置 arena
// buffer_a / buffer_b / dir_map 由各阶段完整覆盖，无需清零
pub fn reset(&mut self)
```

**字段说明：**

| 字段               | 类型       | 说明                       |
| ------------------ | ---------- | -------------------------- |
| `width` / `height` | `usize`    | 图像尺寸                   |
| `capacity`         | `usize`    | `width * height`           |
| `buffer_a`         | `Vec<f32>` | 梯度幅值缓冲区             |
| `buffer_b`         | `Vec<f32>` | 高斯平滑输出缓冲区         |
| `dir_map`          | `Vec<u8>`  | 梯度方向图（值域 0/1/2/3） |
| `edge_map`         | `Vec<u8>`  | 最终边缘图（值域 0/255）   |
| `arena`            | `Bump`     | BFS 零分配 Arena           |

---

### `CannyConfig` / `CannyConfigBuilder`

```rust
pub struct CannyConfig {
    pub sigma:       f32,  // 高斯 sigma，<= 0 跳过高斯
    pub low_thresh:  f32,  // NMS 低阈值
    pub high_thresh: f32,  // NMS 高阈值（必须 >= low_thresh）
}

CannyConfig::builder()
    .sigma(f32)
    .thresholds(low: f32, high: f32)
    .build() -> Result<CannyConfig, CannyError>
```

### `canny()` / `canny_u8()`

```rust
// f32 输入
pub fn canny<'ws>(
    src: &[f32],
    ws:  &'ws mut CannyWorkspace,
    cfg: &CannyConfig,
) -> Result<&'ws [u8], CannyError>

// u8 输入（内部自动转换，无额外堆分配）
pub fn canny_u8<'ws>(
    src: &[u8],
    ws:  &'ws mut CannyWorkspace,
    cfg: &CannyConfig,
) -> Result<&'ws [u8], CannyError>
```

返回的 `&[u8]` 生命周期绑定到 `ws`，下次调用 `canny()`/`canny_u8()` 或 `ws.reset()` 前有效。

---

### `CannyError`

| 变体                                       | 触发条件                      |
| ------------------------------------------ | ----------------------------- |
| `InvalidDimensions { width, height }`      | `width < 3` 或 `height < 3`   |
| `InputLengthMismatch { expected, actual }` | `src.len() != width * height` |
| `InvalidThresholds { low, high }`          | `low_thresh > high_thresh`    |
| `NullPointer`                              | C-FFI 传入空指针              |

---

### `SobelKernel` trait（内核扩展接口）

```rust
pub trait SobelKernel: Send + Sync {
    fn process_row_slice(
        &self,
        src:     &[f32],
        mag_out: &mut [f32],
        dir_out: &mut [u8],
        width:   usize,
        x_start: usize,  // >= 1
        x_end:   usize,  // <= width - 1
        y:       usize,  // >= 1 且 <= height - 2
    );
}
```

可通过实现此 trait 接入自定义硬件后端。

---

### `gaussian::apply`

```rust
pub fn apply(
    src:    &[f32],
    dst:    &mut [f32],
    width:  usize,
    height: usize,
    sigma:  f32,   // 必须 > 0
)
```

委托 `libblur::gaussian_blur_f32` 实现，使用 `EdgeMode::Clamp` 边界处理。

---

## 示例程序

### `detect_image` — 对真实图片执行边缘检测

```bash
# 基本用法（输出自动命名为 input_edge.jpg）
cargo run --example detect_image -- input.jpg

# 完整参数
cargo run --example detect_image -- input.png output_edge.png 1.0 50.0 150.0
```

参数说明：

| 参数          | 默认值  | 说明                         |
| ------------- | ------- | ---------------------------- |
| `<输入路径>`  | 必填    | 支持 png/jpg/bmp/tiff/webp   |
| `[输出路径]`  | 自动生成 | 未指定时为 `<stem>_edge.<ext>` |
| `[sigma]`     | `1.0`   | 高斯平滑强度，`0.0` 跳过     |
| `[low_thresh]`  | `50.0`  | NMS 低阈值                   |
| `[high_thresh]` | `150.0` | NMS 高阈值                   |

输出示例：

```
=== fast-canny edge detection ===
input:       input.jpg
output:      input_edge.jpg
sigma:       1
low_thresh:  50
high_thresh: 150

✓ image loaded: 1920x1080 (2073600 pixels)
✓ workspace created (2.3 ms)
✓ canny done (4.71 ms) — edge pixels: 38420 (1.85%)
✓ result saved: input_edge.jpg

total elapsed: 8.12 ms
```

---

### `visual_demo` — 多维度视觉验证

生成 10 类合成测试图像并执行边缘检测，同时进行阈值和 sigma 敏感性扫描：

```bash
cargo run --example visual_demo
```

输出目录：`target/visual_demo/`，每个用例生成三张图：

| 文件后缀      | 内容                     |
| ------------- | ------------------------ |
| `*_src.png`   | 原始输入图（归一化灰度） |
| `*_edge.png`  | 二值边缘图（0/255）      |
| `*_overlay.png` | 红色边缘叠加在原图上   |

验证用例列表：

| 编号 | 场景           | 预期结果         |
| ---- | -------------- | ---------------- |
| 01   | 均匀图像       | 零边缘           |
| 02   | 垂直阶跃       | 单列边缘         |
| 03   | 水平阶跃       | 单行边缘         |
| 04   | 棋盘格（32px） | 网格边缘         |
| 05   | 圆形           | 闭合曲线         |
| 06   | 噪声图（高阈值）| 稀疏边缘        |
| 07   | 噪声图（低阈值）| 密集边缘        |
| 08   | 线性渐变       | 无边缘（低梯度） |
| 09   | 同心矩形框     | 多条平行边缘     |
| 10   | 最小尺寸 3×3   | 不 panic         |

额外扫描：
- **阈值敏感性**：棋盘格图像，5 档阈值（very_low → very_high），验证边缘数单调递减
- **Sigma 敏感性**：圆形图像，5 档 sigma（0.0 → 4.0），验证平滑越强边缘越少
- **多帧复用**：同一 workspace 交替处理阶跃图和均匀图，验证 `reset()` 正确性

---

## 测试与验证

### 运行测试

```bash
# 运行所有单元测试 + 集成测试
cargo test

# 显示 log 输出（含 trace/debug 级别）
cargo test -- --nocapture

# 仅运行集成测试
cargo test --test integration

# 运行指定模块
cargo test canny_u8_tests
cargo test nms_boundary_tests
cargo test hysteresis_tests
cargo test determinism_tests
cargo test threshold_behavior_tests
cargo test special_input_tests
```

### 测试模块说明

**`canny_u8_tests`**（6 个用例）

| 用例 | 验证内容 |
| ---- | -------- |
| `test_canny_u8_uniform_no_edges` | 均匀 u8 图像输出零边缘 |
| `test_canny_u8_output_binary` | 输出仅含 0/255 |
| `test_canny_u8_step_image_has_edges` | 阶跃图像产生边缘 |
| `test_canny_u8_matches_f32_path` | u8 路径与 f32 路径结果一致（sigma=0） |
| `test_canny_u8_length_mismatch` | 输入长度不匹配返回正确错误 |
| `test_canny_u8_invalid_thresholds` | 非法阈值返回正确错误 |

**`nms_boundary_tests`**（2 个用例）

| 用例 | 验证内容 |
| ---- | -------- |
| `test_boundary_pixels_always_zero` | 四周边界像素 hysteresis 后必须为 0 |
| `test_step_edges_not_on_boundary` | 阶跃边缘仅出现在内部区域 |

**`hysteresis_tests`**（3 个用例）

| 用例 | 验证内容 |
| ---- | -------- |
| `test_no_strong_seeds_yields_no_edges` | 无强边缘种子时输出全为 0 |
| `test_no_weak_edges_in_output` | 输出中不存在值为 127 的弱边缘 |
| `test_equal_thresholds_no_weak_edges` | low==high 时输出严格二值 |

**`determinism_tests`**（2 个用例）

| 用例 | 验证内容 |
| ---- | -------- |
| `test_deterministic_output` | 同一输入多次调用结果完全一致 |
| `test_interleaved_inputs_deterministic` | 不同输入交替调用结果与单独调用一致 |

**`threshold_behavior_tests`**（2 个用例）

| 用例 | 验证内容 |
| ---- | -------- |
| `test_higher_threshold_fewer_or_equal_edges` | 阈值越高边缘数单调不增 |
| `test_zero_low_thresh_keeps_all_gradient_pixels` | low=0 保留所有梯度像素 |

**`special_input_tests`**（5 个用例）

| 用例 | 验证内容 |
| ---- | -------- |
| `test_all_zero_input` | 全零输入不 panic，输出全 0 |
| `test_all_max_input` | 全 255 输入不 panic，输出全 0 |
| `test_horizontal_line_has_edges` | 单像素宽水平线产生边缘 |
| `test_checkerboard_edge_count_vs_cell_size` | 格子越大边缘数越少 |
| `test_larger_sigma_fewer_or_equal_edges` | sigma 越大边缘数单调不增 |

### Debug 构建额外检查

`cargo test`（debug profile）时自动启用以下运行时断言：

- NMS 方向值越界检测（`dir > 3` 时记录 error 并跳过）
- NMS x 范围合法性检查
- `track_edges` 边界清零自验证
- 边缘密度过高警告（> 20% 时提示调高阈值）

---

## 性能基准

在 Intel Core i7-12700K（AVX2+FMA 启用）上的参考数据：

| 分辨率    | 帧率（Release） | 内存分配/帧     |
| --------- | --------------- | --------------- |
| 640×480   | ~2000 fps       | 0（Arena 复用） |
| 1920×1080 | ~400 fps        | 0（Arena 复用） |
| 3840×2160 | ~90 fps         | 0（Arena 复用） |

> 以上数据为估算值，实际性能取决于 CPU 型号、内存带宽和阈值配置。

---
