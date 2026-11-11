// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The Spix XML-RPC test server for a GPUI app written in Rust, the twin of `spix_slint`.
//!
//! The protocol is `spix_server`'s; this crate is its GPUI backend. [`start`] takes the windows
//! to serve by name, and the calls are handed to GPUI's foreground thread, where they run as
//! futures on its executor. A path is `<window>/<id>/<id>/...`: the first segment names a
//! registered window and the rest are element ids, matched against the registry that GPUI Kit's
//! `test-support` feature fills while a frame paints (see the README for the exact rule).
//!
//! What lives only on the foreground thread (the app handle, the windows, the registered inputs
//! and hooks) is kept in thread-locals set up by [`start`] and the `register_*` and `on_*`
//! functions, which therefore must be called on that thread, as an app's setup does.

mod backend;
mod gui;
mod input;
mod lookup;
mod screen;

#[cfg(test)]
extern crate self as spix_gpui;

#[cfg(test)]
mod test_ui;

use gpui_kit::component::input::InputState;
use gpui_kit::{AnyWindowHandle, App, Entity, Window};

pub use spix_server::Error;

/// Starts a Spix server on `127.0.0.1:port`, serving `windows` under their names: the first
/// segment of every path. Call it on the GPUI foreground thread, from the app's setup, with the
/// windows opened; the calls it gets are handed to that thread.
///
/// # Errors
///
/// If nothing can listen on `port`, or the thread that answers the calls can't be started.
pub fn start<'a, W: Into<AnyWindowHandle>>(
    port: u16,
    windows: impl IntoIterator<Item = (&'a str, W)>,
    cx: &mut App,
) -> Result<(), Error> {
    let tasks = gui::attach(
        cx,
        windows
            .into_iter()
            .map(|(name, window)| (name.to_owned(), window.into())),
    );
    spix_server::start(backend::GpuiBackend::new(tasks), port)
}

/// Makes `state` the input behind the elements `path` spells, so that `setStringProperty` of
/// `text` on one sets its value. The registry reads an input's value but can't write it.
///
/// `path` is a Spix path below the window, as tests spell it: `"roomComposer/composerField"`, or
/// `"composerField"` when the id is unique. Inputs may share an own id. An element takes the input
/// whose path matches its own by the rule of the paths (see the README); when two do, nothing is
/// set, and the call is an error in `getErrors`. Registering a path again replaces its input.
pub fn register_input(path: &str, state: Entity<InputState>) {
    gui::register_input(path, state);
}

/// Answers properties the registry can't, such as the `count` of a list, which only its model
/// knows because a virtual list paints only the rows in view. `hook` gets the path of the element
/// (the named segments of its registry path from the root down, spelled as a Spix path spells
/// them, its own id last, so that it can tell same-named elements in different scopes apart; for
/// the window itself, just the window's name) and the name of the property, and answers `None`
/// when it has nothing to say. It is asked for `text`, `checked`
/// and `enabled` only when the registry has no answer. One hook: a second call replaces it.
pub fn on_property(hook: impl Fn(&mut App, &[String], &str) -> Option<String> + 'static) {
    gui::set_property_hook(std::rc::Rc::new(hook));
}

/// Takes the properties set with `setStringProperty` that are not the `text` of a registered
/// input: `hook` gets the path and the property as [`on_property`] does, and the new value, and
/// answers whether it took it. Without a hook, or when it answers `false`, the call is an error
/// in `getErrors`. One hook: a second call replaces it.
pub fn on_set_property(
    hook: impl Fn(&mut Window, &mut App, &[String], &str, &str) -> bool + 'static,
) {
    gui::set_set_hook(std::rc::Rc::new(hook));
}
