// tests/stride_test.rs
mod common;

use doc_quad::core::buffer::DocBuffer;
use doc_quad::find_document;

/// T2：带 Padding 步长（stride > width）场景验证
///
/// 模拟 Android CameraX 在某些分辨率下产生的行对齐填充。
/// 验证 stride 紧凑化逻辑正确剥离了 Padding 字节，不影响检测结果。
#[test]
fn test_strided_buffer_detection() {
    common::init_test_log();

    let width = 640u32;
    let height = 480u32;
    // 模拟带 Padding 的步长（每行末尾有 60 字节填充）
    let stride = 700u32;

    // 创建一个位于中央的实心矩形（300x200），背景 50，内部 200
    let data = common::create_mock_y_channel(width, height, stride, (100, 100), (400, 300));

    let buffer =
        DocBuffer::new(&data, width, height, stride).expect("Failed to create buffer");

    let result = find_document(&buffer).expect("Detection process failed");

    assert!(result.is_some(), "Should detect the rectangular document");

    if let Some(quad) = result {
        // 验证面积（理论值 300*200=60000，允许 Canny 边缘偏移约 ±1px 的误差）
        assert!(
            quad.area > 50000.0 && quad.area < 70000.0,
            "Area out of range: {}",
            quad.area
        );

        // 验证四个顶点的大致位置（允许 ±10px 的边缘检测误差）
        // points 顺序由 Transformer::sort_points 保证：[左上, 右上, 右下, 左下]
        let [tl, tr, br, bl] = quad.points;

        // 左上角：预期 (100, 100)
        assert!(tl.x > 90.0 && tl.x < 110.0, "TL.x out of range: {}", tl.x);
        assert!(tl.y > 90.0 && tl.y < 110.0, "TL.y out of range: {}", tl.y);

        // 右上角：预期 (400, 100)
        assert!(tr.x > 390.0 && tr.x < 410.0, "TR.x out of range: {}", tr.x);
        assert!(tr.y > 90.0 && tr.y < 110.0, "TR.y out of range: {}", tr.y);

        // 右下角：预期 (400, 300)
        assert!(br.x > 390.0 && br.x < 410.0, "BR.x out of range: {}", br.x);
        assert!(br.y > 290.0 && br.y < 310.0, "BR.y out of range: {}", br.y);

        // 左下角：预期 (100, 300)
        assert!(bl.x > 90.0 && bl.x < 110.0, "BL.x out of range: {}", bl.x);
        assert!(bl.y > 290.0 && bl.y < 310.0, "BL.y out of range: {}", bl.y);
    }
}

/// T1：连续内存（stride == width）场景验证
///
/// 验证无 Padding 场景下算法能正确识别文档，
/// 同时验证顶点排序与面积计算的基本正确性。
#[test]
fn test_contiguous_buffer_detection() {
    common::init_test_log();

    let width = 320u32;
    let height = 240u32;
    let stride = width; // 无 Padding

    // 创建实心矩形（220x140），背景 50，内部 200
    let data = common::create_mock_y_channel(width, height, stride, (50, 50), (270, 190));

    let buffer =
        DocBuffer::new(&data, width, height, stride).expect("Failed to create buffer");

    let result = find_document(&buffer).expect("Detection process failed");

    assert!(result.is_some(), "Should detect document in contiguous buffer");

    if let Some(quad) = result {
        // 验证面积（理论值 220*140=30800，允许 ±5000 的边缘检测误差）
        assert!(
            quad.area > 25000.0 && quad.area < 36000.0,
            "Area out of range: {}",
            quad.area
        );

        // 验证四个顶点的大致位置（允许 ±10px 误差）
        let [tl, tr, br, bl] = quad.points;

        // 左上角：预期 (50, 50)
        assert!(tl.x > 40.0 && tl.x < 60.0, "TL.x out of range: {}", tl.x);
        assert!(tl.y > 40.0 && tl.y < 60.0, "TL.y out of range: {}", tl.y);

        // 右上角：预期 (270, 50)
        assert!(tr.x > 260.0 && tr.x < 280.0, "TR.x out of range: {}", tr.x);
        assert!(tr.y > 40.0 && tr.y < 60.0, "TR.y out of range: {}", tr.y);

        // 右下角：预期 (270, 190)
        assert!(br.x > 260.0 && br.x < 280.0, "BR.x out of range: {}", br.x);
        assert!(br.y > 180.0 && br.y < 200.0, "BR.y out of range: {}", br.y);

        // 左下角：预期 (50, 190)
        assert!(bl.x > 40.0 && bl.x < 60.0, "BL.x out of range: {}", bl.x);
        assert!(bl.y > 180.0 && bl.y < 200.0, "BL.y out of range: {}", bl.y);
    }
}

/// T3：非法输入应返回 InvalidBuffer 错误
#[test]
fn test_buffer_overflow_protection() {
    // stride < width：非法布局
    let data = vec![0u8; 100];
    assert!(
        DocBuffer::new(&data, 10, 10, 9).is_err(),
        "stride < width should fail"
    );

    // height * stride > data.len()：数据不足
    assert!(
        DocBuffer::new(&data, 10, 10, 11).is_err(),
        "insufficient data should fail"
    );

    // 合法输入应成功
    let data = vec![0u8; 100];
    assert!(
        DocBuffer::new(&data, 10, 10, 10).is_ok(),
        "valid buffer should succeed"
    );
}
