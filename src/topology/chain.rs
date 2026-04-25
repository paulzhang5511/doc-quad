// src/topology/chain.rs

/// 8-邻域方向定义（顺时针，从"右"开始）：
/// 0=右, 1=右下, 2=下, 3=左下, 4=左, 5=左上, 6=上, 7=右上
const DIRS: [(i32, i32); 8] = [
    (1, 0),   // 0: 右
    (1, 1),   // 1: 右下
    (0, 1),   // 2: 下
    (-1, 1),  // 3: 左下
    (-1, 0),  // 4: 左
    (-1, -1), // 5: 左上
    (0, -1),  // 6: 上
    (1, -1),  // 7: 右上
];

/// 从当前像素 (cx, cy) 出发，从 search_dir 方向开始顺时针搜索，
/// 返回第一个有效的边缘邻域像素坐标及其方向索引。
///
/// # 参数
/// - `edges`：Canny 二值图（0 或 255）
/// - `width`、`height`：图像尺寸
/// - `cx`、`cy`：当前像素坐标
/// - `search_dir`：起始搜索方向（0~7）
///
/// # 返回
/// `Some((nx, ny, dir))`：找到的邻域坐标及方向；`None`：无可用邻域。
pub fn find_next_pixel(
    edges: &[u8],
    width: usize,
    height: usize,
    cx: usize,
    cy: usize,
    search_dir: usize,
) -> Option<(usize, usize, usize)> {
    for i in 0..8 {
        let dir = (search_dir + i) % 8;
        let (dx, dy) = DIRS[dir];

        let nx = cx as i32 + dx;
        let ny = cy as i32 + dy;

        // 边界检查（跳过图像边界像素）
        if nx <= 0 || ny <= 0 || nx >= (width as i32 - 1) || ny >= (height as i32 - 1) {
            continue;
        }

        let nx = nx as usize;
        let ny = ny as usize;

        if edges[ny * width + nx] != 0 {
            return Some((nx, ny, dir));
        }
    }
    None
}

/// 返回给定方向的反方向（对角方向）。
///
/// 方向编号为顺时针 0~7，反方向即 (dir + 4) % 8。
pub fn reverse_dir(dir: usize) -> usize {
    (dir + 4) % 8
}
