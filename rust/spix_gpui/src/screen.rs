// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! Pixels of a window, taken from the X server. GPUI draws a window on the GPU and has no way to
//! read the picture back on Linux (`Window::render_to_image` is not implemented for its windows),
//! so the server asks X for the contents of the window, as a screenshot tool does. That needs a
//! window on an X server: not Wayland, and not GPUI's headless platform, which draws nothing.

use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use spix_server::WindowImage;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, ImageOrder};

/// An image of the whole window, in physical pixels.
pub(crate) fn window_image(window: &Window) -> Result<WindowImage, String> {
    let id = x11_window(window)?;
    let (width, height, rgba) = grab(id)?;
    Ok(WindowImage {
        width,
        height,
        scale: f64::from(window.scale_factor()),
        rgba,
    })
}

/// The X id of the window, from the raw handle GPUI's window gives out.
fn x11_window(window: &Window) -> Result<u32, String> {
    // Not `window.window_handle()`: `Window` has an inherent method of that name, for its GPUI
    // handle.
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|_| "the window has no native handle: GPUI's headless platform draws nothing")?;
    match handle.as_raw() {
        RawWindowHandle::Xcb(handle) => Ok(handle.window.get()),
        RawWindowHandle::Xlib(handle) => u32::try_from(handle.window)
            .map_err(|_| "the X window id does not fit 32 bits".to_owned()),
        other => Err(format!(
            "screenshots need an X11 window, this one is {other:?}"
        )),
    }
}

/// The pixels of the X window `id` as red, green, blue and an opaque alpha.
fn grab(id: u32) -> Result<(u32, u32, Vec<u8>), String> {
    let failed = |error: &dyn std::fmt::Display| format!("the X server refused the grab: {error}");
    let (connection, _) = x11rb::connect(None).map_err(|error| failed(&error))?;
    let geometry = connection
        .get_geometry(id)
        .map_err(|error| failed(&error))?
        .reply()
        .map_err(|error| failed(&error))?;
    let (width, height) = (u32::from(geometry.width), u32::from(geometry.height));

    // Only the layout every desktop X server has is read: 32 bits a pixel, blue first.
    let setup = connection.setup();
    let depth = geometry.depth;
    let packed = setup
        .pixmap_formats
        .iter()
        .any(|format| format.depth == depth && format.bits_per_pixel == 32);
    if !packed || setup.image_byte_order != ImageOrder::LSB_FIRST {
        return Err(format!(
            "the X server's pixel layout is not supported (depth {depth})"
        ));
    }

    let image = connection
        .get_image(
            ImageFormat::Z_PIXMAP,
            id,
            0,
            0,
            geometry.width,
            geometry.height,
            u32::MAX,
        )
        .map_err(|error| failed(&error))?
        .reply()
        .map_err(|error| failed(&error))?;
    let wanted = usize::try_from(width * height * 4).map_err(|error| failed(&error))?;
    if image.data.len() < wanted {
        return Err(format!(
            "the X server sent {} bytes for {width}x{height} pixels",
            image.data.len()
        ));
    }
    let mut rgba = Vec::with_capacity(wanted);
    for pixel in image.data[..wanted].as_chunks::<4>().0 {
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], u8::MAX]);
    }
    Ok((width, height, rgba))
}
