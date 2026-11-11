// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! A backend for the tests of the server: windows with named elements at fixed places, and a log
//! of what the server asked of it. It runs a GUI task on the spot, and its sleeps really sleep (a
//! few milliseconds at most in the tests) so that the deadlines of a gesture mean what they do in
//! an app.

use std::collections::HashMap;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use crate::{Backend, GuiGone, GuiTask, Modifier, Point, PointerEvent, Rect, WindowImage};

/// One window of the fake: where each element is, by the path below the window.
pub(crate) struct Window {
    pub(crate) elements: HashMap<String, Rect>,
    /// The size of the window itself, from its top-left corner at the origin.
    pub(crate) size: (f64, f64),
}

#[derive(Default)]
pub(crate) struct Fake {
    pub(crate) windows: HashMap<String, Window>,
    /// What the server did, one line per primitive.
    pub(crate) log: Mutex<Vec<String>>,
    /// The image of every window, if the fake has pixels.
    pub(crate) image: Option<WindowImage>,
    /// Properties by element path, then name. Settable ones answer true when set.
    pub(crate) properties: Mutex<HashMap<(String, String), String>>,
}

impl Fake {
    /// A fake with one window, `main`, 200 by 100, holding the elements of `elements`.
    pub(crate) fn with(elements: &[(&str, Rect)]) -> Self {
        let mut fake = Self::default();
        fake.windows.insert(
            "main".to_owned(),
            Window {
                elements: elements
                    .iter()
                    .map(|(name, rect)| ((*name).to_owned(), *rect))
                    .collect(),
                size: (200.0, 100.0),
            },
        );
        fake
    }

    pub(crate) fn log(&self) -> Vec<String> {
        self.log.lock().map(|log| log.clone()).unwrap_or_default()
    }

    fn note(&self, line: String) {
        if let Ok(mut log) = self.log.lock() {
            log.push(line);
        }
    }
}

pub(crate) const BUTTON: Rect = Rect {
    x: 10.0,
    y: 20.0,
    width: 40.0,
    height: 10.0,
};

impl Backend for Fake {
    /// The path below the window, empty for the window itself.
    type Item = String;

    const TOOLKIT: &'static str = "Fake";

    fn spawn_on_gui(&self, task: GuiTask) -> Result<(), GuiGone> {
        let mut future = task();
        let mut context = Context::from_waker(Waker::noop());
        // The fake's futures finish in one poll: its sleeps block instead of waiting.
        match future.as_mut().poll(&mut context) {
            Poll::Ready(()) => Ok(()),
            Poll::Pending => Err(GuiGone),
        }
    }

    fn quit(&self) {
        self.note("quit".to_owned());
    }

    fn find(&self, window: &str, names: &[&str]) -> Option<String> {
        let window = self.windows.get(window)?;
        let path = names.join("/");
        (path.is_empty() || window.elements.contains_key(&path)).then_some(path)
    }

    fn frame(&self, window: &str, item: &String) -> Option<Rect> {
        let window = self.windows.get(window)?;
        if item.is_empty() {
            return Some(Rect {
                x: 0.0,
                y: 0.0,
                width: window.size.0,
                height: window.size.1,
            });
        }
        window.elements.get(item).copied()
    }

    fn bounds(&self, window: &str, item: &String) -> Option<Rect> {
        // The window sits at (1000, 500) on the screen.
        let frame = self.frame(window, item)?;
        Some(Rect {
            x: frame.x + 1000.0,
            y: frame.y + 500.0,
            ..frame
        })
    }

    fn visible(&self, _window: &str, item: &String) -> bool {
        item != "invisible"
    }

    fn property(&self, _window: &str, item: &String, name: &str) -> Option<String> {
        self.properties
            .lock()
            .ok()?
            .get(&(item.clone(), name.to_owned()))
            .cloned()
    }

    fn set_property(&self, _window: &str, item: &String, name: &str, value: &str) -> bool {
        if name != "text" {
            return false;
        }
        if let Ok(mut properties) = self.properties.lock() {
            properties.insert((item.clone(), name.to_owned()), value.to_owned());
        }
        true
    }

    fn pointer(&self, window: &str, at: Point, event: PointerEvent) -> bool {
        if !self.windows.contains_key(window) {
            return false;
        }
        let event = match event {
            PointerEvent::Press(button) => format!("press {button:?}"),
            PointerEvent::Release(button) => format!("release {button:?}"),
            PointerEvent::Move => "move".to_owned(),
            PointerEvent::Wheel { angle_x, angle_y } => format!("wheel {angle_x},{angle_y}"),
        };
        self.note(format!("{event} at {},{}", at.x, at.y));
        true
    }

    fn modifier(&self, _window: &str, modifier: Modifier, pressed: bool) {
        let direction = if pressed { "down" } else { "up" };
        self.note(format!("{modifier:?} {direction}"));
    }

    fn input_text(&self, window: &str, text: &str) {
        self.note(format!("text {text:?} to {window}"));
    }

    fn enter_key(&self, window: &str, key_code: i32, modifiers: &[Modifier]) {
        self.note(format!("key {key_code:#x} {modifiers:?} to {window}"));
    }

    async fn sleep(&self, duration: Duration) {
        self.note(format!("sleep {}", duration.as_millis()));
        std::thread::sleep(duration);
    }

    fn window_image(&self, _window: &str) -> Result<WindowImage, String> {
        self.image.clone().ok_or_else(|| "no pixels".to_owned())
    }
}
