// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The Slint backend of the server: the windows an app registered, and the primitives of
//! [`spix_server::Backend`] over them.
//!
//! The registry lives on the GUI thread, where [`start`](crate::start) is called, and is only ever
//! touched there: every call hops to it, [`SlintBackend::quit`] excepted because it touches
//! nothing. The listener thread hands a task over with [`slint::invoke_from_event_loop`], and the
//! task runs as a future with [`slint::spawn_local`], because a click awaits Slint's own timers,
//! which only fire while the event loop runs.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::task::{Poll, Waker};
use std::time::Duration;

use i_slint_backend_testing::{ElementHandle, ElementRoot};
use slint::{ComponentHandle, Timer, Weak, Window};
use spix_server::{
    Backend, Button, Clicks, GuiGone, GuiTask, Modifier, Point, PointerEvent, Rect, WindowImage,
};

use crate::input;
use crate::scene::{self, Item};

thread_local! {
    /// The windows the server answers for. Set by [`register`] on the GUI thread and only ever
    /// touched there.
    static WINDOWS: RefCell<Vec<(String, Box<dyn WindowSource>)>> = const { RefCell::new(Vec::new()) };
    /// The frame counters of the windows whose `frameCount` was read, by the address of the
    /// `Window`: a component's window lives as long as the component, and the counter with it.
    /// Apart from [`WINDOWS`], as they are read and added while a call holds that to find the window.
    static FRAMES: RefCell<Vec<(usize, Rc<Cell<u64>>)>> = const { RefCell::new(Vec::new()) };
}

/// One window of the app, with the type of its component erased.
trait WindowSource {
    /// Runs `f` with the window of the component and the root element to search from, while the
    /// component is still alive. False once it is gone.
    fn open(&self, f: &mut dyn FnMut(&Window, &ElementHandle) -> bool) -> bool;
}

/// The `Weak` of a component a caller handed to [`start`](crate::start).
struct Registered<T: ComponentHandle>(Weak<T>);

impl<T: ComponentHandle> WindowSource for Registered<T> {
    fn open(&self, f: &mut dyn FnMut(&Window, &ElementHandle) -> bool) -> bool {
        let Some(component) = self.0.upgrade() else {
            return false;
        };
        f(component.window(), &component.root_element())
    }
}

/// Serves `window` under `name`: the first segment of every path that is meant for it.
pub(crate) fn register<T: ComponentHandle + 'static>(name: &str, window: Weak<T>) {
    WINDOWS.with(|windows| {
        windows
            .borrow_mut()
            .push((name.to_owned(), Box::new(Registered(window))));
    });
}

/// Runs `f` with the window registered under `name` and the root element to search from, while the
/// component behind it is still alive. False when there is no such window.
fn with_window(name: &str, f: &mut dyn FnMut(&Window, &ElementHandle) -> bool) -> bool {
    WINDOWS.with(|windows| {
        let windows = windows.borrow();
        let Some((_, window)) = windows.iter().find(|(registered, _)| registered == name) else {
            return false;
        };
        window.open(f)
    })
}

/// Runs `f` with the window `name`, and returns what it answered, or `None` when the window is
/// gone.
fn read<T>(name: &str, mut f: impl FnMut(&Window) -> T) -> Option<T> {
    let mut answer = None;
    with_window(name, &mut |window, _| {
        answer = Some(f(window));
        true
    });
    answer
}

/// The windows of the registry, for the server to serve.
pub(crate) struct SlintBackend;

impl Backend for SlintBackend {
    type Item = Item;

    const TOOLKIT: &'static str = "Slint";

    fn spawn_on_gui(&self, task: GuiTask) -> Result<(), GuiGone> {
        slint::invoke_from_event_loop(move || {
            // A task that can't be spawned is dropped, which the listener sees as a gone event
            // loop. Can't happen: the event loop proxy `invoke_from_event_loop` just used is still
            // there.
            let _ = slint::spawn_local(task());
        })
        .map_err(|_| GuiGone)
    }

    fn quit(&self) {
        let _ = slint::quit_event_loop();
    }

    fn find(&self, window: &str, names: &[&str]) -> Option<Item> {
        let mut found = None;
        with_window(window, &mut |_, root| {
            found = scene::find_item(root, names);
            found.is_some()
        });
        found
    }

    fn frame(&self, window: &str, item: &Item) -> Option<Rect> {
        read(window, |window| scene::frame(window, item))
    }

    fn bounds(&self, window: &str, item: &Item) -> Option<Rect> {
        read(window, |window| scene::bounds(window, item))
    }

    fn visible(&self, window: &str, item: &Item) -> bool {
        read(window, |window| scene::visible(window, item)).unwrap_or(false)
    }

    /// What the scene knows, and the frame count of a window, which lives here with the rendering
    /// notifiers.
    fn property(&self, window: &str, item: &Item, name: &str) -> Option<String> {
        read(window, |window| match (item, name) {
            (Item::Window, "frameCount") => frame_count(window),
            _ => scene::string_property(item, name),
        })?
    }

    fn set_property(&self, _window: &str, item: &Item, name: &str, value: &str) -> bool {
        scene::set_string_property(item, name, value)
    }

    fn pointer(&self, window: &str, at: Point, event: PointerEvent) -> bool {
        read(window, |window| input::pointer(window, logical(at), event)).is_some()
    }

    fn modifier(&self, window: &str, modifier: Modifier, pressed: bool) {
        read(window, |window| {
            let key = input::modifier_key(modifier);
            if pressed {
                input::press(window, key);
            } else {
                input::release(window, key);
            }
        });
    }

    fn input_text(&self, window: &str, text: &str) {
        read(window, |window| input::input_text(window, text));
    }

    fn enter_key(&self, window: &str, key_code: i32, modifiers: &[Modifier]) {
        read(window, |window| {
            // A key Slint has no key for is dropped, as C++ Spix drops it.
            let _ = input::enter_key(window, key_code, modifiers);
        });
    }

    /// Waits on a Slint timer, which only fires while the event loop runs.
    async fn sleep(&self, duration: Duration) {
        let state = Rc::new(RefCell::new((false, None::<Waker>)));
        let timer_state = Rc::clone(&state);
        Timer::single_shot(duration, move || {
            let mut state = timer_state.borrow_mut();
            state.0 = true;
            if let Some(waker) = state.1.take() {
                waker.wake();
            }
        });
        std::future::poll_fn(move |context| {
            let mut state = state.borrow_mut();
            if state.0 {
                Poll::Ready(())
            } else {
                state.1 = Some(context.waker().clone());
                Poll::Pending
            }
        })
        .await;
    }

    fn window_image(&self, window: &str) -> Result<WindowImage, String> {
        read(window, scene::snapshot).unwrap_or_else(|| Err("the window is gone".to_owned()))
    }

    /// Slint's own `single_click` and `double_click`, which know where an element inside an open
    /// popup stands: its `absolute_position()` is relative to the popup.
    async fn click(&self, _window: &str, item: &Item, button: Button, clicks: Clicks) {
        let Item::Element(element) = item else {
            return;
        };
        let button = input::slint_button(button);
        match clicks {
            Clicks::Single => element.single_click(button).await,
            Clicks::Double => element.double_click(button).await,
        }
    }
}

/// A position of the server's, as Slint's logical position.
#[allow(clippy::cast_possible_truncation)] // Pixels are small.
fn logical(at: Point) -> slint::LogicalPosition {
    slint::LogicalPosition::new(at.x as f32, at.y as f32)
}

/// The frames `window` has drawn since the first time this was asked, which is 0. Counting starts
/// there, with a rendering notifier on the window, so an app whose tests never ask pays nothing.
///
/// `None` when the window can't be counted: its renderer has no notifier support (the testing
/// backend's software renderer), or the app already installed a notifier, a window having one.
fn frame_count(window: &Window) -> Option<String> {
    let key = std::ptr::from_ref(window) as usize;
    let known = FRAMES.with(|frames| {
        frames
            .borrow()
            .iter()
            .find(|(counted, _)| *counted == key)
            .map(|(_, frames)| frames.get())
    });
    if let Some(frames) = known {
        return Some(frames.to_string());
    }
    let frames = Rc::new(Cell::new(0_u64));
    let counter = Rc::clone(&frames);
    window
        .set_rendering_notifier(move |state, _api| {
            if matches!(state, slint::RenderingState::AfterRendering) {
                counter.set(counter.get() + 1);
            }
        })
        .ok()?;
    FRAMES.with(|counters| counters.borrow_mut().push((key, frames)));
    Some("0".to_owned())
}
