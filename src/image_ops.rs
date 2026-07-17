use image::{imageops, DynamicImage, ImageDecoder, ImageReader, ImageResult, Rgba, RgbaImage};
use std::{collections::VecDeque, path::Path};

pub fn load_oriented_rgba(path: &Path) -> ImageResult<RgbaImage> {
    let mut decoder = ImageReader::open(path)?.into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image.to_rgba8())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushMode {
    Erase,
    Restore,
}

#[inline]
fn color_distance_rgb(pixel: &Rgba<u8>, color: [u8; 3]) -> f32 {
    let dr = pixel[0] as f32 - color[0] as f32;
    let dg = pixel[1] as f32 - color[1] as f32;
    let db = pixel[2] as f32 - color[2] as f32;
    (dr * dr + dg * dg + db * db).sqrt()
}

pub fn smart_edge_remove(image: &mut RgbaImage, tolerance: u8) -> usize {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return 0;
    }

    let corners = [
        image.get_pixel(0, 0),
        image.get_pixel(w - 1, 0),
        image.get_pixel(0, h - 1),
        image.get_pixel(w - 1, h - 1),
    ]
    .map(|p| [p[0], p[1], p[2]]);

    let tol = tolerance as f32 * 1.73;
    let len = (w as usize).saturating_mul(h as usize);
    let mut visited = vec![false; len];
    let mut queue = VecDeque::with_capacity(len.min(1_000_000));

    let qualifies = |idx: usize, img: &RgbaImage| {
        let x = (idx % w as usize) as u32;
        let y = (idx / w as usize) as u32;
        let pixel = img.get_pixel(x, y);
        corners
            .iter()
            .any(|corner| color_distance_rgb(pixel, *corner) <= tol)
    };

    let add = |idx: usize, visited: &mut [bool], queue: &mut VecDeque<usize>, img: &RgbaImage| {
        if !visited[idx] && qualifies(idx, img) {
            visited[idx] = true;
            queue.push_back(idx);
        }
    };

    for x in 0..w as usize {
        add(x, &mut visited, &mut queue, image);
        add(
            (h as usize - 1) * w as usize + x,
            &mut visited,
            &mut queue,
            image,
        );
    }
    for y in 0..h as usize {
        add(y * w as usize, &mut visited, &mut queue, image);
        add(
            y * w as usize + w as usize - 1,
            &mut visited,
            &mut queue,
            image,
        );
    }

    let mut count = 0usize;
    while let Some(idx) = queue.pop_front() {
        count += 1;
        let x = idx % w as usize;
        let y = idx / w as usize;
        if x > 0 {
            add(idx - 1, &mut visited, &mut queue, image);
        }
        if x + 1 < w as usize {
            add(idx + 1, &mut visited, &mut queue, image);
        }
        if y > 0 {
            add(idx - w as usize, &mut visited, &mut queue, image);
        }
        if y + 1 < h as usize {
            add(idx + w as usize, &mut visited, &mut queue, image);
        }
    }

    for (idx, was_visited) in visited.into_iter().enumerate() {
        if was_visited {
            let x = (idx % w as usize) as u32;
            let y = (idx / w as usize) as u32;
            image.get_pixel_mut(x, y)[3] = 0;
        }
    }
    count
}

pub fn remove_picked_color(image: &mut RgbaImage, picked: [u8; 3], tolerance: u8) -> usize {
    let tol = tolerance as f32 * 1.73;
    let mut count = 0usize;
    for pixel in image.pixels_mut() {
        if color_distance_rgb(pixel, picked) <= tol {
            pixel[3] = 0;
            count += 1;
        }
    }
    count
}

pub fn soften_alpha(image: &mut RgbaImage, radius: u32) {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 || radius == 0 {
        return;
    }

    // Summed-area table makes the box blur O(width * height), even for larger radii.
    let stride = w as usize + 1;
    let mut integral = vec![0u64; (w as usize + 1) * (h as usize + 1)];
    for y in 0..h as usize {
        let mut row_sum = 0u64;
        for x in 0..w as usize {
            row_sum += image.get_pixel(x as u32, y as u32)[3] as u64;
            integral[(y + 1) * stride + (x + 1)] = integral[y * stride + (x + 1)] + row_sum;
        }
    }

    let mut out = vec![0u8; (w as usize) * (h as usize)];
    for y in 0..h as usize {
        let y0 = y.saturating_sub(radius as usize);
        let y1 = (y + radius as usize + 1).min(h as usize);
        for x in 0..w as usize {
            let x0 = x.saturating_sub(radius as usize);
            let x1 = (x + radius as usize + 1).min(w as usize);
            let sum = integral[y1 * stride + x1] + integral[y0 * stride + x0]
                - integral[y0 * stride + x1]
                - integral[y1 * stride + x0];
            let n = (x1 - x0) * (y1 - y0);
            out[y * w as usize + x] = ((sum as f64 / n as f64).round()) as u8;
        }
    }

    for (pixel, alpha) in image.pixels_mut().zip(out) {
        pixel[3] = alpha;
    }
}

pub fn threshold_alpha(image: &mut RgbaImage, threshold: u8) {
    for pixel in image.pixels_mut() {
        pixel[3] = if pixel[3] < threshold { 0 } else { 255 };
    }
}

pub fn apply_brush_line(
    image: &mut RgbaImage,
    original: &RgbaImage,
    from: (f32, f32),
    to: (f32, f32),
    radius: f32,
    mode: BrushMode,
) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let distance = (dx * dx + dy * dy).sqrt();
    let spacing = (radius * 0.35).max(1.0);
    let steps = (distance / spacing).ceil().max(1.0) as usize;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = from.0 + dx * t;
        let y = from.1 + dy * t;
        apply_brush_stamp(image, original, x, y, radius, mode);
    }
}

pub fn apply_brush_stamp(
    image: &mut RgbaImage,
    original: &RgbaImage,
    cx: f32,
    cy: f32,
    radius: f32,
    mode: BrushMode,
) {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return;
    }
    let radius = radius.max(0.5);
    let r2 = radius * radius;
    let x0 = (cx - radius).floor().max(0.0) as u32;
    let y0 = (cy - radius).floor().max(0.0) as u32;
    let x1 = (cx + radius).ceil().min(w.saturating_sub(1) as f32) as u32;
    let y1 = (cy + radius).ceil().min(h.saturating_sub(1) as f32) as u32;

    for y in y0..=y1 {
        for x in x0..=x1 {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let dx = px - cx;
            let dy = py - cy;
            if dx * dx + dy * dy <= r2 {
                match mode {
                    BrushMode::Erase => image.get_pixel_mut(x, y)[3] = 0,
                    BrushMode::Restore => *image.get_pixel_mut(x, y) = *original.get_pixel(x, y),
                }
            }
        }
    }
}

pub fn crop_rgba(image: &RgbaImage, x: u32, y: u32, width: u32, height: u32) -> RgbaImage {
    let x = x.min(image.width().saturating_sub(1));
    let y = y.min(image.height().saturating_sub(1));
    let width = width.max(1).min(image.width().saturating_sub(x));
    let height = height.max(1).min(image.height().saturating_sub(y));
    imageops::crop_imm(image, x, y, width, height).to_image()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_edge_only_removes_connected_background() {
        let mut img = RgbaImage::from_pixel(5, 5, Rgba([255, 255, 255, 255]));
        for y in 1..4 {
            for x in 1..4 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        img.put_pixel(2, 2, Rgba([255, 255, 255, 255]));
        let removed = smart_edge_remove(&mut img, 0);
        assert_eq!(removed, 16);
        assert_eq!(img.get_pixel(0, 0)[3], 0);
        assert_eq!(img.get_pixel(2, 2)[3], 255);
    }

    #[test]
    fn picked_color_removes_every_match() {
        let mut img = RgbaImage::from_pixel(2, 1, Rgba([255, 255, 255, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        assert_eq!(remove_picked_color(&mut img, [255, 255, 255], 0), 1);
        assert_eq!(img.get_pixel(0, 0)[3], 0);
        assert_eq!(img.get_pixel(1, 0)[3], 255);
    }

    #[test]
    fn restore_brush_restores_original_rgba() {
        let original = RgbaImage::from_pixel(3, 3, Rgba([10, 20, 30, 200]));
        let mut working = original.clone();
        working.get_pixel_mut(1, 1)[3] = 0;
        apply_brush_stamp(&mut working, &original, 1.5, 1.5, 1.0, BrushMode::Restore);
        assert_eq!(*working.get_pixel(1, 1), Rgba([10, 20, 30, 200]));
    }

    #[test]
    fn threshold_is_binary() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 100]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 200]));
        threshold_alpha(&mut img, 128);
        assert_eq!(img.get_pixel(0, 0)[3], 0);
        assert_eq!(img.get_pixel(1, 0)[3], 255);
    }
}
