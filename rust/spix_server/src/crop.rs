// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! Cropping an element out of an image of its window, for `takeScreenshot`.

use crate::{Rect, WindowImage};

/// Why an item can't be photographed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CropError {
    /// The backend has no pixels, as a renderer that only mocks the drawing.
    #[error("the window can't be rendered: {0}")]
    Image(String),
    #[error("the item has no visible area in the window")]
    Empty,
    #[error("can't encode the image: {0}")]
    Encode(#[from] png::EncodingError),
}

/// A PNG of `frame`, cropped from `image`. The image is in physical pixels and the frame in
/// logical ones, so the frame is scaled and clamped to the image.
pub(crate) fn png(image: &WindowImage, frame: Rect) -> Result<Vec<u8>, CropError> {
    let edge = |logical: f64, limit: u32| {
        // Clamped to the image first, so the conversion back to u32 can't overflow or go negative.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let pixel = (logical * image.scale).round().clamp(0.0, f64::from(limit)) as u32;
        pixel
    };
    let left = edge(frame.x, image.width);
    let top = edge(frame.y, image.height);
    let right = edge(frame.x + frame.width, image.width);
    let bottom = edge(frame.y + frame.height, image.height);
    if right <= left || bottom <= top {
        return Err(CropError::Empty);
    }

    // Four bytes a pixel, row after row: copy the rows of the crop.
    let row = |y: u32| {
        let start = (y as usize * image.width as usize + left as usize) * 4;
        &image.rgba[start..start + (right - left) as usize * 4]
    };
    let mut cropped = Vec::with_capacity(((right - left) * (bottom - top)) as usize * 4);
    for y in top..bottom {
        cropped.extend_from_slice(row(y));
    }

    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, right - left, bottom - top);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&cropped)?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4x2 image at scale 2 whose pixel at column `x`, row `y` is `[x, y, 0, 255]`.
    fn image() -> WindowImage {
        let mut rgba = Vec::new();
        for y in 0..2_u8 {
            for x in 0..4_u8 {
                rgba.extend_from_slice(&[x, y, 0, 255]);
            }
        }
        WindowImage {
            width: 4,
            height: 2,
            scale: 2.0,
            rgba,
        }
    }

    fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        pixels.truncate(info.buffer_size());
        (info.width, info.height, pixels)
    }

    #[test]
    fn crops_the_frame_scaled_to_physical_pixels() {
        // Logical 1..2 by 0..1 is physical columns 2..4, rows 0..2.
        let frame = Rect {
            x: 1.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        let (width, height, pixels) = decode(&png(&image(), frame).unwrap());
        assert_eq!((width, height), (2, 2));
        assert_eq!(
            pixels,
            [2, 0, 0, 255, 3, 0, 0, 255, 2, 1, 0, 255, 3, 1, 0, 255]
        );
    }

    #[test]
    fn clamps_a_frame_that_sticks_out_of_the_image() {
        let frame = Rect {
            x: -5.0,
            y: -5.0,
            width: 100.0,
            height: 100.0,
        };
        let (width, height, _) = decode(&png(&image(), frame).unwrap());
        assert_eq!((width, height), (4, 2));
    }

    #[test]
    fn a_frame_outside_the_image_has_no_area() {
        let frame = Rect {
            x: 10.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        assert!(matches!(png(&image(), frame), Err(CropError::Empty)));
    }
}
