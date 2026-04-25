// examples/detect_image.rs
//
// 用法：
//   cargo run --release --example detect_image -- <input> [output] [debug_dir]
//
// 参数：
//   <input>      输入图片路径（jpg/png/bmp 等）
//   [output]     输出结果图路径（默认 <stem>_result.png）
//   [debug_dir]  调试中间图输出目录（默认 <stem>_debug/）
//                设为 "none" 可禁用调试图输出

use doc_quad::core::buffer::DocBuffer;
use doc_quad::edge::detector::EdgeDetector;
use doc_quad::find_document;
use image::{GenericImageView, GrayImage, Rgb, RgbImage};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn main() {
    // 初始化日志，默认 debug 级别，可通过 RUST_LOG 覆盖
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format_timestamp_millis()
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input> [output] [debug_dir|none]", args[0]);
        std::process::exit(1);
    }

    let input_path = Path::new(&args[1]);
    let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();

    let output_path: PathBuf = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("{}_result.png", stem)));

    let debug_dir: Option<PathBuf> = match args.get(3).map(|s| s.as_str()) {
        Some("none") => None,
        Some(p) => Some(PathBuf::from(p)),
        None => Some(PathBuf::from(format!("{}_debug", stem))),
    };

    println!("=== DocQuad Document Detection ===");
    println!("Input:     {}", input_path.display());
    println!("Output:    {}", output_path.display());
    println!(
        "Debug dir: {}",
        debug_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "disabled".to_string())
    );
    println!();

    // ── 创建调试目录 ──────────────────────────────────────────────────────────
    if let Some(ref dir) = debug_dir {
        if let Err(e) = std::fs::create_dir_all(dir) {
            log::warn!(
                "[detect_image] - Failed to create debug dir {}: {}",
                dir.display(),
                e
            );
        } else {
            log::info!("[detect_image] - Debug dir created: {}", dir.display());
        }
    }

    // ── 加载图像 ──────────────────────────────────────────────────────────────
    let load_start = Instant::now();
    let img = match image::open(input_path) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Failed to open image: {}", e);
            std::process::exit(1);
        }
    };

    let (img_width, img_height) = img.dimensions();
    println!("Image size: {}x{}", img_width, img_height);
    log::info!(
        "[detect_image] - Image loaded: {}x{}, color_type={:?}. Elapsed: {}ms",
        img_width,
        img_height,
        img.color(),
        load_start.elapsed().as_millis()
    );

    // 转换为灰度图
    let gray = img.to_luma8();

    // 保存原始灰度图（调试用）
    if let Some(ref dir) = debug_dir {
        let gray_path = dir.join("debug_00_input_gray.png");
        if let Err(e) = gray.save(&gray_path) {
            log::warn!("[detect_image] - Failed to save input gray: {}", e);
        } else {
            log::info!("[detect_image] - Saved input gray: {}", gray_path.display());
        }

        // 同时保存原始彩色图的缩略图（便于对照）
        let thumb_long = 512u32;
        let (tw, th) = if img_width > img_height {
            (
                thumb_long,
                (img_height as f32 * thumb_long as f32 / img_width as f32) as u32,
            )
        } else {
            (
                (img_width as f32 * thumb_long as f32 / img_height as f32) as u32,
                thumb_long,
            )
        };
        let thumb = img.thumbnail(tw, th);
        let thumb_path = dir.join("debug_00_thumbnail.png");
        if let Err(e) = thumb.save(&thumb_path) {
            log::warn!("[detect_image] - Failed to save thumbnail: {}", e);
        } else {
            log::info!(
                "[detect_image] - Saved thumbnail ({}x{}): {}",
                tw,
                th,
                thumb_path.display()
            );
        }
    }

    let gray_data: Vec<u8> = gray.into_raw();

    // ── 构建 DocBuffer ────────────────────────────────────────────────────────
    let buffer = match DocBuffer::new(&gray_data, img_width, img_height, img_width) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to create DocBuffer: {}", e);
            std::process::exit(1);
        }
    };

    // ── 单独执行边缘检测（带调试图输出）──────────────────────────────────────
    // 注意：find_document 内部会再次执行完整流水线（含下采样），
    // 这里单独对下采样后的数据执行带调试的边缘检测，输出中间图。
    log::info!("[detect_image] - Running debug edge detection on downsampled image...");

    // 手动执行下采样（与 lib.rs 中逻辑一致）
    const DOWNSAMPLE_THRESHOLD: u32 = 1024 * 768;
    const TARGET_LONG_EDGE: u32 = 1024;

    let total_pixels = img_width * img_height;
    let scale = if total_pixels > DOWNSAMPLE_THRESHOLD {
        let long_edge = img_width.max(img_height);
        TARGET_LONG_EDGE as f32 / long_edge as f32
    } else {
        1.0
    };

    let (proc_width, proc_height, proc_data) = if scale < 1.0 {
        let w = ((img_width as f32 * scale) as u32).max(3);
        let h = ((img_height as f32 * scale) as u32).max(3);
        log::info!(
            "[detect_image] - Downsampling {}x{} -> {}x{} (scale={:.4})",
            img_width,
            img_height,
            w,
            h,
            scale
        );
        let data = downsample_nearest(&gray_data, img_width, img_height, img_width, w, h);
        (w, h, data)
    } else {
        (img_width, img_height, gray_data.clone())
    };

    // 保存下采样后的灰度图
    if let Some(ref dir) = debug_dir {
        let ds_path = dir.join("debug_01_downsampled.png");
        if let Some(ds_img) = GrayImage::from_raw(proc_width, proc_height, proc_data.clone()) {
            if let Err(e) = ds_img.save(&ds_path) {
                log::warn!("[detect_image] - Failed to save downsampled: {}", e);
            } else {
                log::info!(
                    "[detect_image] - Saved downsampled ({}x{}): {}",
                    proc_width,
                    proc_height,
                    ds_path.display()
                );
            }
        }
    }

    // 对下采样图执行带调试的边缘检测
    let proc_buffer = match DocBuffer::new(&proc_data, proc_width, proc_height, proc_width) {
        Ok(b) => b,
        Err(e) => {
            log::error!("[detect_image] - Failed to create proc_buffer: {}", e);
            std::process::exit(1);
        }
    };

    let mut detector = match EdgeDetector::new(proc_width as usize, proc_height as usize) {
        Ok(d) => d,
        Err(e) => {
            log::error!("[detect_image] - Failed to create EdgeDetector: {}", e);
            std::process::exit(1);
        }
    };

    // 执行带调试输出的边缘检测
    let edge_result = detector.detect_with_debug(&proc_buffer, debug_dir.as_deref());

    match edge_result {
        Ok(ref edges) => {
            // 保存边缘图的 PNG 版本（比 PGM 更通用）
            if let Some(ref dir) = debug_dir {
                save_edge_overlay(
                    edges,
                    &proc_data,
                    proc_width as usize,
                    proc_height as usize,
                    &dir.join("debug_07_edge_overlay.png"),
                );
                log::info!("[detect_image] - Saved edge overlay to debug dir.");
            }
        }
        Err(e) => {
            log::error!("[detect_image] - Debug edge detection failed: {}", e);
        }
    }

    // ── 执行完整检测流水线 ────────────────────────────────────────────────────
    println!("\n--- Running full detection pipeline ---");
    let detect_start = Instant::now();

    match find_document(&buffer) {
        Ok(Some(quad)) => {
            let elapsed = detect_start.elapsed().as_millis();
            println!("Result: Document FOUND! Elapsed: {}ms", elapsed);
            println!("  Area:   {:.0} px²", quad.area);
            println!("  TL: ({:.1}, {:.1})", quad.points[0].x, quad.points[0].y);
            println!("  TR: ({:.1}, {:.1})", quad.points[1].x, quad.points[1].y);
            println!("  BR: ({:.1}, {:.1})", quad.points[2].x, quad.points[2].y);
            println!("  BL: ({:.1}, {:.1})", quad.points[3].x, quad.points[3].y);

            log::info!(
                "[detect_image] - Detection SUCCESS: area={:.0}px², \
                 TL=({:.1},{:.1}), TR=({:.1},{:.1}), BR=({:.1},{:.1}), BL=({:.1},{:.1}). \
                 Elapsed: {}ms",
                quad.area,
                quad.points[0].x,
                quad.points[0].y,
                quad.points[1].x,
                quad.points[1].y,
                quad.points[2].x,
                quad.points[2].y,
                quad.points[3].x,
                quad.points[3].y,
                elapsed
            );

            // 在原图上绘制检测结果并保存
            let result_img = draw_quad_on_image(&image::open(input_path).unwrap(), &quad.points);
            if let Err(e) = result_img.save(&output_path) {
                log::error!("[detect_image] - Failed to save result image: {}", e);
            } else {
                println!("Saved: {}", output_path.display());
            }

            // 在下采样图上也绘制（便于与调试边缘图对照）
            if let Some(ref dir) = debug_dir {
                let scaled_pts = quad
                    .points
                    .map(|p| glam::Vec2::new(p.x * scale, p.y * scale));
                if let Ok(ds_img_orig) = image::open(input_path) {
                    let ds_result = draw_quad_on_image(
                        &ds_img_orig.thumbnail(proc_width, proc_height),
                        &scaled_pts,
                    );
                    let ds_result_path = dir.join("debug_08_detection_result_downsampled.png");
                    if let Err(e) = ds_result.save(&ds_result_path) {
                        log::warn!("[detect_image] - Failed to save ds result: {}", e);
                    } else {
                        log::info!(
                            "[detect_image] - Saved ds result: {}",
                            ds_result_path.display()
                        );
                    }
                }
            }
        }
        Ok(None) => {
            let elapsed = detect_start.elapsed().as_millis();
            println!("Result: No document detected. Elapsed: {}ms", elapsed);
            log::warn!(
                "[detect_image] - Detection FAILED: no document found. Elapsed: {}ms",
                elapsed
            );

            // 即使未检测到，也保存一张空白结果图（便于确认图像加载正常）
            if let Ok(orig) = image::open(input_path) {
                let blank = orig.to_rgb8();
                if let Err(e) = blank.save(&output_path) {
                    log::error!("[detect_image] - Failed to save blank result: {}", e);
                } else {
                    println!("Saved (original, no detection): {}", output_path.display());
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            log::error!("[detect_image] - Detection error: {}", e);
            std::process::exit(1);
        }
    }

    // ── 输出调试文件清单 ──────────────────────────────────────────────────────
    if let Some(ref dir) = debug_dir {
        println!("\n--- Debug files saved to: {} ---", dir.display());
        println!("  debug_00_thumbnail.png          原始图缩略图");
        println!("  debug_00_input_gray.png         原始灰度图");
        println!(
            "  debug_01_downsampled.png         下采样后灰度图 ({}x{})",
            proc_width, proc_height
        );
        println!("  debug_01_input_gray.pgm          传入 Canny 的灰度图");
        println!(
            "  debug_02_canny_raw.pgm           Canny 原始边缘图 (low={:.1}, high={:.1}, sigma=1.0)",
            5.0, 20.0
        );
        println!("  debug_03_after_close.pgm         形态学闭运算后 (radius=1)");
        println!("  debug_04_close_radius2.pgm       形态学闭运算后 (radius=2)");
        println!("  debug_05_close_radius3.pgm       形态学闭运算后 (radius=3)");
        println!("  debug_06_sigma05_close_r2.pgm    sigma=0.5 + radius=2 组合");
        println!("  debug_07_edge_overlay.png        边缘叠加图（红色边缘）");
        println!("  debug_08_detection_result_downsampled.png  检测结果（下采样尺寸）");
        println!(
            "\n  查看 .pgm 文件：可用 GIMP、ImageMagick (magick xxx.pgm xxx.png) 或 IrfanView 打开"
        );
    }
}

/// 最近邻下采样（与 lib.rs 中逻辑保持一致）
fn downsample_nearest(
    data: &[u8],
    src_width: u32,
    src_height: u32,
    stride: u32,
    target_w: u32,
    target_h: u32,
) -> Vec<u8> {
    let mut out = Vec::with_capacity((target_w * target_h) as usize);
    let stride = stride as usize;

    for ty in 0..target_h {
        let sy = ((ty as f32 + 0.5) * src_height as f32 / target_h as f32) as usize;
        let sy = sy.min(src_height as usize - 1);
        let row_offset = sy * stride;

        for tx in 0..target_w {
            let sx = ((tx as f32 + 0.5) * src_width as f32 / target_w as f32) as usize;
            let sx = sx.min(src_width as usize - 1);
            out.push(data[row_offset + sx]);
        }
    }
    out
}

/// 将边缘图（白色边缘）叠加到灰度图上，生成红色边缘可视化图。
fn save_edge_overlay(edges: &[u8], gray: &[u8], width: usize, height: usize, path: &Path) {
    let mut rgb = RgbImage::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let g = gray[idx];
            if edges[idx] == 255 {
                // 边缘像素：红色
                rgb.put_pixel(x as u32, y as u32, Rgb([255u8, 0, 0]));
            } else {
                // 非边缘：原始灰度
                rgb.put_pixel(x as u32, y as u32, Rgb([g, g, g]));
            }
        }
    }

    match rgb.save(path) {
        Ok(_) => log::info!("[detect_image] - Saved edge overlay: {}", path.display()),
        Err(e) => log::warn!(
            "[detect_image] - Failed to save edge overlay {}: {}",
            path.display(),
            e
        ),
    }
}

/// 在图像上绘制四边形轮廓（绿色线条 + 角点标记）。
fn draw_quad_on_image(img: &image::DynamicImage, points: &[glam::Vec2; 4]) -> RgbImage {
    let mut rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();

    // 绘制四条边（绿色，线宽 3px）
    let edges = [(0, 1), (1, 2), (2, 3), (3, 0)];
    for (i, j) in edges {
        let p0 = points[i];
        let p1 = points[j];
        draw_line_thick(
            &mut rgb,
            p0.x as i32,
            p0.y as i32,
            p1.x as i32,
            p1.y as i32,
            Rgb([0u8, 255, 0]),
            3,
        );
    }

    // 绘制角点（不同颜色区分：TL=红, TR=蓝, BR=黄, BL=青）
    let colors = [
        Rgb([255u8, 0, 0]),   // TL 红
        Rgb([0u8, 0, 255]),   // TR 蓝
        Rgb([255u8, 255, 0]), // BR 黄
        Rgb([0u8, 255, 255]), // BL 青
    ];
    let labels = ["TL", "TR", "BR", "BL"];

    for (i, (&pt, &color)) in points.iter().zip(colors.iter()).enumerate() {
        let cx = pt.x as i32;
        let cy = pt.y as i32;
        // 绘制 9×9 实心方块标记角点
        for dy in -4i32..=4 {
            for dx in -4i32..=4 {
                let px = (cx + dx).clamp(0, w as i32 - 1) as u32;
                let py = (cy + dy).clamp(0, h as i32 - 1) as u32;
                rgb.put_pixel(px, py, color);
            }
        }
        log::debug!(
            "[detect_image] - Quad corner {}: ({:.1}, {:.1})",
            labels[i],
            pt.x,
            pt.y
        );
    }

    rgb
}

/// Bresenham 直线算法，支持指定线宽。
fn draw_line_thick(
    img: &mut RgbImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Rgb<u8>,
    thickness: i32,
) {
    let (w, h) = img.dimensions();
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1i32 } else { -1 };
    let sy = if y0 < y1 { 1i32 } else { -1 };
    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        // 绘制以 (x,y) 为中心的 thickness×thickness 方块
        let half = thickness / 2;
        for dy in -half..=half {
            for dx in -half..=half {
                let px = (x + dx).clamp(0, w as i32 - 1) as u32;
                let py = (y + dy).clamp(0, h as i32 - 1) as u32;
                img.put_pixel(px, py, color);
            }
        }

        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}
