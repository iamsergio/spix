// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The GPUI backend of the server: [`spix_server::Backend`] over the windows an app registered.
//!
//! Every method but [`spawn_on_gui`](GpuiBackend::spawn_on_gui) and
//! [`quit`](GpuiBackend::quit) runs on the foreground thread, in a task that the listener sent
//! there, and reaches the windows through [`gui::with_window`].

use std::time::Duration;

use gpui_kit::base::test_support::ElementSnapshot;
use gpui_kit::component::input::InputState;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{Bounds, ElementId, Entity, Pixels};
use spix_server::{
    Backend, Button, Clicks, GuiGone, GuiTask, Modifier, Point, PointerEvent, Rect, WindowImage,
};

use crate::lookup::{self, Item};
use crate::{gui, input, screen};

/// The tasks go to the foreground thread through this channel, which a loop that [`gui::attach`]
/// started there drains.
pub(crate) struct GpuiBackend {
    tasks: async_channel::Sender<GuiTask>,
}

impl GpuiBackend {
    pub(crate) fn new(tasks: async_channel::Sender<GuiTask>) -> Self {
        Self { tasks }
    }
}

fn rect(bounds: Bounds<Pixels>) -> Rect {
    Rect {
        x: f64::from(f32::from(bounds.origin.x)),
        y: f64::from(f32::from(bounds.origin.y)),
        width: f64::from(f32::from(bounds.size.width)),
        height: f64::from(f32::from(bounds.size.height)),
    }
}

fn truth(value: bool) -> String {
    value.to_string()
}

/// What the registry says of an element, for the properties it has a say in: the `text` is the
/// value of an input, or the label an app gave (`aria_label`) to anything else.
fn registry_property(snapshot: &ElementSnapshot, name: &str) -> Option<String> {
    match name {
        "text" => snapshot
            .value()
            .or_else(|| snapshot.label())
            .map(str::to_owned),
        "checked" => Some(truth(snapshot.checked().unwrap_or(false))),
        "enabled" => Some(truth(!snapshot.disabled().unwrap_or(false))),
        _ => None,
    }
}

/// The registered input whose path matches the element at `path` (spelled `id`): `None` if there
/// is none, and `Err` if several match, which is never guessed. The clashing paths go to stderr,
/// since the server words the error for the property and doesn't know them.
fn registered_input(path: &[ElementId], id: &[String]) -> Result<Option<Entity<InputState>>, ()> {
    let mut matching: Vec<_> = gui::inputs()
        .into_iter()
        .filter(|(names, _)| {
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            lookup::matches(path, &names)
        })
        .collect();
    if matching.len() > 1 {
        eprintln!(
            "spix_gpui: the text of {} matches {} registered inputs: {:?}",
            id.join("/"),
            matching.len(),
            matching
                .iter()
                .map(|(names, _)| names.join("/"))
                .collect::<Vec<_>>()
        );
        return Err(());
    }
    Ok(matching.pop().map(|(_, state)| state))
}

impl Backend for GpuiBackend {
    type Item = Item;

    const TOOLKIT: &'static str = "GPUI";

    fn spawn_on_gui(&self, task: GuiTask) -> Result<(), GuiGone> {
        self.tasks.try_send(task).map_err(|_| GuiGone)
    }

    fn quit(&self) {
        // Hops like a call does: `App::quit` is for the foreground thread, and `quit` is called
        // from the listener.
        let _ = self.tasks.try_send(Box::new(|| {
            Box::pin(async {
                gui::with_app(|app| app.quit());
            })
        }));
    }

    fn find(&self, window: &str, names: &[&str]) -> Option<Item> {
        gui::with_window(window, |window, cx| {
            if names.is_empty() {
                return Some(Item::Window);
            }
            // The registry is filled while a frame paints: draw the one that shows what the
            // last call did.
            window.render_frame(cx);
            lookup::find(window, names)
        })?
    }

    fn frame(&self, window: &str, item: &Item) -> Option<Rect> {
        gui::with_window(window, |window, _| match item {
            Item::Window => Some(rect(Bounds::from_corners(
                gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.)),
                gpui_kit::point(window.viewport_size().width, window.viewport_size().height),
            ))),
            Item::Element(path) => lookup::resolve(window, path).map(|found| rect(found.bounds())),
        })?
    }

    /// The frame, moved by where the window is on the screen.
    fn bounds(&self, window: &str, item: &Item) -> Option<Rect> {
        let frame = self.frame(window, item)?;
        let origin = gui::with_window(window, |window, _| window.bounds().origin)?;
        Some(Rect {
            x: frame.x + f64::from(f32::from(origin.x)),
            y: frame.y + f64::from(f32::from(origin.y)),
            ..frame
        })
    }

    fn visible(&self, window: &str, item: &Item) -> bool {
        gui::with_window(window, |window, _| match item {
            Item::Window => true,
            Item::Element(path) => {
                lookup::resolve(window, path).is_some_and(|found| found.visible())
            }
        })
        .unwrap_or(false)
    }

    fn property(&self, window_name: &str, item: &Item, name: &str) -> Option<String> {
        gui::with_window(window_name, |window, cx| {
            let (id, known) = match item {
                Item::Window => (vec![window_name.to_owned()], None),
                Item::Element(path) => {
                    // The text of a registered input is its value, which the registry doesn't
                    // expose for a masked one. With several matches, or none, the registry
                    // answers.
                    let typed = (name == "text")
                        .then(|| registered_input(path, &lookup::spelled(path)))
                        .and_then(Result::ok)
                        .flatten()
                        .map(|state| state.read(cx).value().to_string());
                    (
                        lookup::spelled(path),
                        typed.or_else(|| {
                            lookup::resolve(window, path)
                                .and_then(|found| registry_property(&found, name))
                        }),
                    )
                }
            };
            known.or_else(|| gui::property_hook().and_then(|hook| hook(cx, &id, name)))
        })?
    }

    fn set_property(&self, window_name: &str, item: &Item, name: &str, value: &str) -> bool {
        gui::with_window(window_name, |window, cx| {
            let id = match item {
                Item::Window => vec![window_name.to_owned()],
                Item::Element(path) => lookup::spelled(path),
            };
            if name == "text"
                && let Item::Element(path) = item
            {
                match registered_input(path, &id) {
                    Err(()) => return false,
                    Ok(Some(state)) => {
                        state.update(cx, |state, cx| {
                            state.set_value(value.to_owned(), window, cx);
                        });
                        window.render_frame(cx);
                        return true;
                    }
                    Ok(None) => {}
                }
            }
            gui::set_hook().is_some_and(|hook| hook(window, cx, &id, name, value))
        })
        .unwrap_or(false)
    }

    fn pointer(&self, window: &str, at: Point, event: PointerEvent) -> bool {
        gui::with_window(window, |window, cx| {
            input::pointer(window, at.x, at.y, event, cx);
        })
        .is_some()
    }

    fn modifier(&self, window: &str, modifier: Modifier, pressed: bool) {
        gui::with_window(window, |window, cx| {
            input::modifier(window, modifier, pressed, cx);
        });
    }

    fn input_text(&self, window: &str, text: &str) {
        gui::with_window(window, |window, cx| input::input_text(window, text, cx));
    }

    fn enter_key(&self, window: &str, key_code: i32, modifiers: &[Modifier]) {
        gui::with_window(window, |window, cx| {
            // A key GPUI has no key for is dropped, as the C++ backend drops it.
            let _ = input::enter_key(window, key_code, modifiers, cx);
        });
    }

    /// Waits on a timer of GPUI's executor, which fires while the app's loop runs.
    async fn sleep(&self, duration: Duration) {
        let Some(app) = gui::app() else {
            return;
        };
        let executor = app.background_executor().clone();
        executor.timer(duration).await;
    }

    fn window_image(&self, window: &str) -> Result<WindowImage, String> {
        gui::with_window(window, |window, cx| {
            // What is on the screen is what the last frame drew: make it the current state.
            window.render_frame(cx);
            screen::window_image(window)
        })
        .unwrap_or_else(|| Err("the window is gone".to_owned()))
    }

    /// A click that counts its clicks in the events, as GPUI wants, at the centre of the item,
    /// popups included: their frames are in window coordinates like the rest.
    async fn click(&self, window: &str, item: &Item, button: Button, clicks: Clicks) {
        let Some(frame) = self.frame(window, item) else {
            return;
        };
        let times = match clicks {
            Clicks::Single => 1,
            Clicks::Double => 2,
        };
        gui::with_window(window, |window, cx| {
            input::click(
                window,
                frame.x + frame.width / 2.0,
                frame.y + frame.height / 2.0,
                button,
                times,
                cx,
            );
        });
    }
}
