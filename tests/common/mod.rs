// tests/common/mod.rs

/// 构造模拟的 YUV Y 通道数据，生成实心填充矩形（矩形内部高亮，背景暗）。
///
/// # 设计说明
/// 使用实心矩形而非单像素边框，原因如下：
/// - 单像素边框会导致 Canny 在边框内外两侧各产生一条边缘线（双线效应），
///   轮廓追踪会提取出 4 条细长轮廓，极值点后备策略因顶点重合而退化失败。
/// - 实心矩形在矩形边界处产生单次亮度阶跃（50→200），Canny 只产生一条边缘线，
///   轮廓追踪能提取出一个干净的闭合矩形轮廓。
///
/// # 参数
/// - `width`、`height`：图像逻辑尺寸（像素）
/// - `stride`：行步长（含 Padding，字节数，stride >= width）
/// - `rect_tl`：矩形左上角坐标 (x, y)（含）
/// - `rect_br`：矩形右下角坐标 (x, y)（不含）
pub fn create_mock_y_channel(
    width: u32,
    height: u32,
    stride: u32,
    rect_tl: (u32, u32),
    rect_br: (u32, u32),
) -> Vec<u8> {
    // 背景填充为暗灰（50），与矩形内部形成明显对比度
    let mut data = vec![50u8; (height * stride) as usize];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * stride + x) as usize;
            // 矩形内部填充为亮灰（200），在边界处形成单次阶跃（50→200）
            // Canny 仅在此阶跃处产生一条边缘线，避免双线效应
            if x >= rect_tl.0 && x < rect_br.0 && y >= rect_tl.1 && y < rect_br.1 {
                data[idx] = 200;
            }
            // 矩形外部保持背景值 50，无需额外赋值
        }
    }
    data
}

/// 初始化测试日志输出（幂等，多次调用安全）
pub fn init_test_log() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Debug)
        .try_init();
}
