// examples/pc_demo.rs
use doc_quad::core::buffer::DocBuffer;
use doc_quad::find_document;
use std::time::Instant;

fn main() {
    // 初始化日志输出，默认输出 debug 及以上级别
    // 可通过环境变量覆盖：RUST_LOG=trace cargo run --example pc_demo
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("debug"),
    )
    .init();

    let width = 800u32;
    let height = 600u32;
    let stride = 800u32;
    let mut raw_pixels = vec![50u8; (width * height) as usize];

    // 在模拟数据中绘制实心矩形（背景 50，内部 200），模拟文档边缘
    // 注意：原始单像素边框会导致 Canny 双线效应，改为实心矩形确保产生单条轮廓
    for y in 100u32..500 {
        for x in 100u32..700 {
            raw_pixels[(y * stride + x) as usize] = 200;
        }
    }

    // 包装为 DocBuffer
    let buffer = DocBuffer::new(&raw_pixels, width, height, stride)
        .expect("Buffer creation failed");

    // 执行检测
    let start = Instant::now();
    match find_document(&buffer) {
        Ok(Some(quad)) => {
            println!("[Demo] - Document found!");
            println!("[Demo] - Area: {:.2}", quad.area);
            for (i, pt) in quad.points.iter().enumerate() {
                println!("  Point {}: x={:.1}, y={:.1}", i, pt.x, pt.y);
            }
        }
        Ok(None) => println!("[Demo] - No document detected."),
        Err(e) => eprintln!("[Demo] - Error: {}", e),
    }
    println!("[Demo] - Total latency: {}ms", start.elapsed().as_millis());
}
