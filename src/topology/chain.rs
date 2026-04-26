// src/topology/chain.rs

/// 8-邻域顺时针搜索方向的坐标偏移 (Freeman Chain Code 规则)
/// 顺序：右(0), 右下(1), 下(2), 左下(3), 左(4), 左上(5), 上(6), 右上(7)
const DIRS: [(i32, i32); 8] = [
    (1, 0),   // 0
    (1, 1),   // 1
    (0, 1),   // 2
    (-1, 1),  // 3
    (-1, 0),  // 4
    (-1, -1), // 5
    (0, -1),  // 6
    (1, -1),  // 7
];

/// 反转方向 180 度
#[inline(always)]
pub fn reverse_dir(dir: u8) -> u8 {
    (dir + 4) % 8
}

/// 在 8 邻域中寻找下一个非零像素。
///
/// # 参数
/// - `edges`: 二值化边缘图像 (1D array)
/// - `width`: 图像宽度
/// - `height`: 图像高度
/// - `cx`, `cy`: 当前坐标
/// - `start_dir`: 起始搜索方向 (0-7)
///
/// # 返回
/// 成功找到则返回 `Some((next_x, next_y, found_dir))`，否则返回 `None`。
pub fn find_next_pixel(
    edges: &[u8],
    width: u32,
    height: u32,
    cx: u32,
    cy: u32,
    start_dir: u8,
) -> Option<(u32, u32, u8)> {
    let cx = cx as i32;
    let cy = cy as i32;
    let w = width as i32;
    let h = height as i32;

    // 从 start_dir 开始，顺时针探查最多 8 个方向
    for step in 0..8 {
        let current_dir = (start_dir + step) % 8;
        let nx = cx + DIRS[current_dir as usize].0;
        let ny = cy + DIRS[current_dir as usize].1;

        // 【核心修复：虚拟 Padding 边界检查】
        // 允许 nx, ny 达到 0 或 w-1, h-1。
        // 如果越界（< 0 或 >= 宽/高），我们直接视为遇到了背景，跳过该方向。
        if nx >= 0 && nx < w && ny >= 0 && ny < h {
            let idx = (ny * w + nx) as usize;
            // 只要不是纯黑，就是边缘
            if edges[idx] != 0 {
                return Some((nx as u32, ny as u32, current_dir));
            }
        }
    }

    None
}
