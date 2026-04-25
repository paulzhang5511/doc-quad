## DocQuad 项目组织结构

```text
doc-quad/
├── Cargo.toml                # 项目元数据与依赖配置 (glam, fast-canny, ndarray, bytemuck, geo)
├── README.md                 # 项目愿景、技术栈说明及快速入门指南
├── ARCHITECTURE.md           # 详细设计文档：内存安全规范、算法流程、坐标系定义
├── .gitignore                # Git 忽略文件配置
├── src/                      # 源代码核心目录
│   ├── lib.rs                # 库入口点，定义公共 API (如 find_document) 及模块导出
│   ├── prelude.rs            # 预导入模块，导出常用的 Buffer、Point、Quadrilateral 等类型
│   ├── error.rs              # 统一错误处理：定义 DocQuadError 枚举（涵盖内存、几何、算法错误）
│   │
│   ├── core/                 # 核心抽象层：处理内存布局与零拷贝映射
│   │   ├── mod.rs            # 模块入口，定义核心数据结构抽象
│   │   ├── buffer.rs         # DocBuffer 定义：封装原始指针/切片，管理数据生命周期
│   │   └── view.rs           # ndarray 适配器：利用 bytemuck 将原始内存映射为带 Stride 的 ArrayView2
│   │
│   ├── edge/                 # 边缘检测算子层：图像到二值图的转换
│   │   ├── mod.rs            # 边缘检测流控制器
│   │   ├── detector.rs       # fast-canny 深度集成：处理从核心视图到 Canny 算子的数据适配
│   │   └── threshold.rs      # 自适应算法：实现基于直方图统计的高低阈值自动计算逻辑
│   │
│   ├── topology/             # 拓扑追踪层：像素点到几何轮廓的转换
│   │   ├── mod.rs            # 轮廓提取接口定义
│   │   ├── contour.rs        # Suzuki-Abe 算法实现：负责扫描边缘图并追踪闭合边界
│   │   └── chain.rs          # 链码与邻域逻辑：处理 8-邻域搜索与边界标记状态机
│   │
│   ├── geom/                 # 几何引擎层：基于数学模型进行分析与拟合
│   │   ├── mod.rs            # 几何筛选与排序主逻辑
│   │   ├── simplify.rs       # 多边形拟合：集成 geo 的 Douglas-Peucker 算法进行轮廓简化
│   │   ├── validate.rs       # 筛选器：执行凸性检测、内角校验及最大面积评分逻辑
│   │   └── transform.rs      # 空间变换：利用 glam 计算单应性矩阵，执行顶点重排 (TL, TR, BR, BL)
│   │
│   └── ffi/                  # 外部函数接口层 (可选)
│       ├── mod.rs            # 导出 C 兼容接口
│       └── android.rs        # 针对 Android JNI 的专用封装逻辑
│
├── tests/                    # 集成测试目录
│   ├── common/               # 测试工具类：包含模拟相机数据生成、断言辅助函数
│   ├── detection_test.rs     # 核心扫描逻辑端到端测试 (使用 image 0.25 读取测试样本)
│   └── stride_test.rs        # 专项测试：验证不同 row_stride 下的内存访问准确性
│
├── examples/                 # 示例代码
│   ├── pc_demo.rs            # PC 端调用示例：读取图片并输出四边形顶点坐标
│   └── stream_processing.rs  # 模拟流式处理示例：演示如何重用中间缓冲区
│
└── benches/                  # 性能基准测试
    ├── canny_bench.rs        # 评估不同分辨率下的边缘检测耗时
    └── contour_bench.rs      # 评估 Suzuki-Abe 算法在复杂边缘下的处理性能
```
