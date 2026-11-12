// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! What lives on GPUI's foreground thread: the app handle, the windows by name, the inputs and
//! hooks an app registered. All of it sits in thread-locals that only the foreground thread
//! touches, since GPUI's `AsyncApp` and entities are not `Send`.

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::{AnyWindowHandle, App, AppContext as _, AsyncApp, Entity, Window};
use spix_server::GuiTask;

/// Answers a property: the window (not to be updated: it is in an update), the app, the element's
/// spelled path, the property name.
pub(crate) type PropertyHook = Rc<dyn Fn(&Window, &mut App, &[String], &str) -> Option<String>>;
/// Sets a property: the window, the app, the element's spelled path, the property name, the value.
pub(crate) type SetHook = Rc<dyn Fn(&mut Window, &mut App, &[String], &str, &str) -> bool>;

struct Gui {
    cx: AsyncApp,
    windows: Vec<(String, AnyWindowHandle)>,
}

/// A text field an app registered: a single-line input or a multi-line textarea, which are
/// different types of GPUI Kit's with the same `value` and `set_value`.
#[derive(Clone)]
pub(crate) enum Field {
    Input(Entity<InputState>),
    Textarea(Entity<TextareaState>),
}

impl Field {
    /// What the field holds, a masked one's included.
    pub(crate) fn value(&self, cx: &App) -> String {
        match self {
            Self::Input(state) => state.read(cx).value().to_string(),
            Self::Textarea(state) => state.read(cx).value().to_string(),
        }
    }

    /// Replaces what the field holds.
    pub(crate) fn set_value(&self, value: &str, window: &mut Window, cx: &mut App) {
        match self {
            Self::Input(state) => {
                state.update(cx, |state, cx| {
                    state.set_value(value.to_owned(), window, cx);
                });
            }
            Self::Textarea(state) => {
                state.update(cx, |state, cx| {
                    state.set_value(value.to_owned(), window, cx);
                });
            }
        }
    }
}

thread_local! {
    static GUI: RefCell<Option<Gui>> = const { RefCell::new(None) };
    static INPUTS: RefCell<Vec<(Vec<String>, Field)>> = const { RefCell::new(Vec::new()) };
    static PROPERTY_HOOK: RefCell<Option<PropertyHook>> = const { RefCell::new(None) };
    static SET_HOOK: RefCell<Option<SetHook>> = const { RefCell::new(None) };
}

/// Remembers the app and the windows, and starts the loop on the foreground that runs the tasks
/// the listener thread sends: it spawns each task's future on the foreground executor, which polls
/// it between the frames. Returns the end of the channel the tasks are sent into.
pub(crate) fn attach(
    cx: &mut App,
    windows: impl Iterator<Item = (String, AnyWindowHandle)>,
) -> async_channel::Sender<GuiTask> {
    let (sender, receiver) = async_channel::unbounded::<GuiTask>();
    GUI.with(|gui| {
        *gui.borrow_mut() = Some(Gui {
            cx: cx.to_async(),
            windows: windows.collect(),
        });
    });
    cx.spawn(async move |cx: &mut AsyncApp| {
        while let Ok(task) = receiver.recv().await {
            cx.foreground_executor().spawn(task()).detach();
        }
    })
    .detach();
    sender
}

/// Registers `state` under the Spix path `path` (below the window). The same path again replaces
/// the input registered under it.
pub(crate) fn register_input(path: &str, state: Field) {
    let names: Vec<String> = path.split('/').map(str::to_owned).collect();
    INPUTS.with(|inputs| {
        let mut inputs = inputs.borrow_mut();
        match inputs.iter_mut().find(|(known, _)| *known == names) {
            Some(entry) => entry.1 = state,
            None => inputs.push((names, state)),
        }
    });
}

/// The registered inputs and the paths they were registered under.
pub(crate) fn inputs() -> Vec<(Vec<String>, Field)> {
    INPUTS.with(|inputs| inputs.borrow().clone())
}

pub(crate) fn set_property_hook(hook: PropertyHook) {
    PROPERTY_HOOK.with(|slot| *slot.borrow_mut() = Some(hook));
}

pub(crate) fn property_hook() -> Option<PropertyHook> {
    PROPERTY_HOOK.with(|slot| slot.borrow().clone())
}

pub(crate) fn set_set_hook(hook: SetHook) {
    SET_HOOK.with(|slot| *slot.borrow_mut() = Some(hook));
}

pub(crate) fn set_hook() -> Option<SetHook> {
    SET_HOOK.with(|slot| slot.borrow().clone())
}

/// The app handle, or `None` before [`attach`].
pub(crate) fn app() -> Option<AsyncApp> {
    GUI.with(|gui| gui.borrow().as_ref().map(|gui| gui.cx.clone()))
}

/// Runs `f` on the app, outside any update that is running.
pub(crate) fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    Some(app()?.update(f))
}

/// Runs `f` with the window registered under `name`, or returns `None` when there is none or it
/// is closed. Cloned out of the thread-local first: `f` may be a hook that calls back into here.
pub(crate) fn with_window<R>(name: &str, f: impl FnOnce(&mut Window, &mut App) -> R) -> Option<R> {
    let (cx, handle) = GUI.with(|gui| {
        let gui = gui.borrow();
        let gui = gui.as_ref()?;
        let (_, handle) = gui.windows.iter().find(|(known, _)| known == name)?;
        Some((gui.cx.clone(), *handle))
    })?;
    cx.update(|app| {
        app.update_window(handle, |_, window, cx| f(window, cx))
            .ok()
    })
}
