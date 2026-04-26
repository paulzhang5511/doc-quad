// src/topology/contour.rs
use glam::Vec2;
use crate::topology::chain::{find_next_pixel, reverse_dir};

pub struct ContourExtractor;

impl ContourExtractor {
    /// 提取二值图像中的外轮廓。
    pub fn extract(edges: &[u8], width: u32, height: u32) -> Vec<Vec<Vec2>> {
        let mut visited = vec![false; (width * height) as usize];
        let mut contours = Vec::new();

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                
                if edges[idx] == 0 || visited[idx] {
                    continue;
                }

                let contour = Self::trace_contour(edges, &mut visited, width, height, x, y);
                
                if contour.len() >= 4 {
                    contours.push(contour);
                }
            }
        }

        contours
    }

    fn trace_contour(
        edges: &[u8],
        visited: &mut [bool],
        width: u32,
        height: u32,
        start_x: u32,
        start_y: u32,
    ) -> Vec<Vec2> {
        let mut contour = Vec::with_capacity(16);
        let mut cx = start_x;
        let mut cy = start_y;
        
        let mut search_dir = 7; 

        contour.push(Vec2::new(cx as f32, cy as f32));
        visited[(cy * width + cx) as usize] = true;

        let mut prev_dir: Option<u8> = None;
        let max_pts = width * height; 

        for _ in 0..max_pts {
            if let Some((nx, ny, found_dir)) = find_next_pixel(edges, width, height, cx, cy, search_dir) {
                visited[(ny * width + nx) as usize] = true;

                if Some(found_dir) == prev_dir && contour.len() > 1 {
                    let last_idx = contour.len() - 1;
                    contour[last_idx] = Vec2::new(nx as f32, ny as f32);
                } else {
                    contour.push(Vec2::new(nx as f32, ny as f32));
                }

                prev_dir = Some(found_dir);

                if nx == start_x && ny == start_y {
                    break; 
                }

                cx = nx;
                cy = ny;
                
                search_dir = (reverse_dir(found_dir) + 1) % 8;
            } else {
                break;
            }
        }

        contour
    }
}