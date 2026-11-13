// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The toolkit-neutral half of the Spix XML-RPC test server: everything that is the protocol
//! (`docs/rpc-api.md`) and nothing that is a GUI toolkit. A backend implements [`Backend`], the
//! handful of primitives a toolkit has to offer, and [`start`] serves the protocol over them.
//!
//! The server owns the XML-RPC codec, the HTTP listener, the faults, the `getErrors` log with the
//! C++ Spix wording, the parameters, `wait`, the path syntax (`<window>/<id>/<id>/...`) and the
//! choreography of the gestures: what a click, a double click, a wheel turn or a drag become as
//! pointer events and pauses, and how an element is cropped out of an image of its window. The
//! backend owns the windows and what it can do with them, and runs on the GUI thread.

mod crop;
mod server;
mod xmlrpc;

#[cfg(test)]
mod fake;

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

pub use server::{Error, start};

/// A task for the GUI thread: a closure that is sent there, and makes the future that is run
/// there. The closure is `Send` and the future is not, since it holds what lives on the GUI thread.
pub type GuiTask = Box<dyn FnOnce() -> LocalTask + Send>;

/// A future that is polled on the GUI thread only.
pub type LocalTask = Pin<Box<dyn Future<Output = ()>>>;

/// The GUI thread can't be reached: the app is on its way out.
#[derive(Debug, thiserror::Error)]
#[error("the GUI thread is gone")]
pub struct GuiGone;

/// A rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A point in logical pixels, from the top-left corner of a window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// A mouse button, from the Spix codes: 1 = left, 2 = right, 4 = middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

impl Button {
    /// The button for a Spix code. None of the three is no button, which the server refuses.
    #[must_use]
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            1 => Some(Self::Left),
            2 => Some(Self::Right),
            4 => Some(Self::Middle),
            _ => None,
        }
    }
}

/// A modifier key, as the bits of Qt's `Qt::KeyboardModifier` carry them: 1 = shift, 2 = control,
/// 4 = alt, 8 = meta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Shift,
    Control,
    Alt,
    Meta,
}

impl Modifier {
    /// The modifiers set in `flags`, in the order they are pressed (and released backwards).
    #[must_use]
    pub fn from_flags(flags: i32) -> Vec<Self> {
        [
            (1, Self::Shift),
            (2, Self::Control),
            (4, Self::Alt),
            (8, Self::Meta),
        ]
        .into_iter()
        .filter(|(flag, _)| flags & flag != 0)
        .map(|(_, modifier)| modifier)
        .collect()
    }
}

/// One pointer event, at a position in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEvent {
    Press(Button),
    Release(Button),
    Move,
    /// A turn of the wheel in Qt's `angleDelta` units (eighths of a degree, 120 to a notch). A
    /// positive y scrolls the content towards its start.
    Wheel {
        angle_x: i32,
        angle_y: i32,
    },
}

/// How many times a click clicks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clicks {
    Single,
    Double,
}

/// An image of a whole window, in physical pixels, for `takeScreenshot` to crop.
#[derive(Debug, Clone)]
pub struct WindowImage {
    pub width: u32,
    pub height: u32,
    /// Physical pixels per logical pixel.
    pub scale: f64,
    /// Four bytes a pixel (red, green, blue, alpha), row after row, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// What the server needs from a GUI toolkit.
///
/// A backend is shared with the listener thread, so it is `Send + Sync`, but every method except
/// [`spawn_on_gui`](Self::spawn_on_gui) and [`quit`](Self::quit) is called on the GUI thread, from
/// a task that [`spawn_on_gui`](Self::spawn_on_gui) runs there, and may use what lives only there.
/// Windows are named by the first segment of a path, and an [`Item`](Self::Item) is whatever a
/// path found in one: an element, or the window itself when the path has no further segment. A
/// method that is told a window that is gone does nothing, and answers as for a missing item.
///
/// The futures are polled on the GUI thread and need not be `Send`.
#[allow(async_fn_in_trait)] // Nothing here leaves the GUI thread, so `Send` would be a burden.
pub trait Backend: Send + Sync + 'static {
    /// What a path found. Cloned across the awaits of a gesture, and never leaves the GUI thread.
    type Item: Clone + 'static;

    /// The toolkit's name, as the error for a property that can't be set names it.
    const TOOLKIT: &'static str;

    /// Hops onto the GUI thread: runs `task` there, and polls the future it makes there until it is
    /// done, with an executor that fires timers while it waits (see [`sleep`](Self::sleep)). Called
    /// from the listener thread.
    ///
    /// # Errors
    ///
    /// When the GUI thread is gone, in which case the task is dropped.
    fn spawn_on_gui(&self, task: GuiTask) -> Result<(), GuiGone>;

    /// Ends the app's event loop so that the app exits normally. Called from any thread.
    fn quit(&self);

    /// The item under `names` in `window`: each name an id among the descendants of the previous
    /// match. With no `names` it is the window itself. None when the window or a name is missing.
    fn find(&self, window: &str, names: &[&str]) -> Option<Self::Item>;

    /// Where `item` is in its window and how big it is, in logical pixels from the window's
    /// top-left corner. This is where pointer events for it land, and what a screenshot crops.
    fn frame(&self, window: &str, item: &Self::Item) -> Option<Rect>;

    /// Where `item` is on the screen, for `getBoundingBox`.
    fn bounds(&self, window: &str, item: &Self::Item) -> Option<Rect>;

    /// Whether the item is visible. A backend whose search only finds visible items answers true
    /// for those and looks at the window itself.
    fn visible(&self, window: &str, item: &Self::Item) -> bool;

    /// The string property `name` of `item`, or None when it has no such property (the server then
    /// answers the empty string). Clients ask for `text`, `checked`, `enabled`, `count` and
    /// `truncated`; a backend's own properties, such as a frame count of the window, go here too.
    fn property(&self, window: &str, item: &Self::Item, name: &str) -> Option<String>;

    /// Sets the string property `name`. False when the item can't take it, which the server
    /// reports in `getErrors`.
    fn set_property(&self, window: &str, item: &Self::Item, name: &str, value: &str) -> bool;

    /// Sends one pointer event at `at` to the window. False when the window is gone.
    fn pointer(&self, window: &str, at: Point, event: PointerEvent) -> bool;

    /// Presses or lets go of a modifier key, to hold it around a click.
    fn modifier(&self, window: &str, modifier: Modifier, pressed: bool);

    /// Types `text`, as a keyboard would, into whatever has the focus in the window.
    fn input_text(&self, window: &str, text: &str);

    /// Presses and releases the key with the Qt `Qt::Key` code `key_code`, holding `modifiers`
    /// around it. A code the toolkit has no key for is dropped.
    fn enter_key(&self, window: &str, key_code: i32, modifiers: &[Modifier]);

    /// Waits `duration` on the GUI thread's own timers, which fire only while its loop runs.
    async fn sleep(&self, duration: Duration);

    /// Lets the toolkit show what it has drawn, before [`window_image`](Self::window_image) reads
    /// the pixels. For a toolkit that presents a frame only when its event loop runs, after the
    /// call that changed the window has returned; the default has nothing to wait for.
    async fn present(&self, _window: &str) {}

    /// An image of the whole window, or why there is none (a renderer without pixels, a window
    /// that can't be grabbed).
    ///
    /// # Errors
    ///
    /// When the window can't be rendered; the string goes into `getErrors`.
    fn window_image(&self, window: &str) -> Result<WindowImage, String>;

    /// Clicks `item` with `button`, once or twice. The default presses and releases at the centre
    /// of its frame; a toolkit that knows better where the item stands (an item in a popup) says
    /// so by overriding it. The click is whole when this is done: the server holds the modifiers
    /// around it.
    async fn click(&self, window: &str, item: &Self::Item, button: Button, clicks: Clicks) {
        let Some(frame) = self.frame(window, item) else {
            return;
        };
        let at = Point {
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height / 2.0,
        };
        let times = match clicks {
            Clicks::Single => 1,
            Clicks::Double => 2,
        };
        for _ in 0..times {
            self.pointer(window, at, PointerEvent::Press(button));
            self.pointer(window, at, PointerEvent::Release(button));
        }
    }
}
