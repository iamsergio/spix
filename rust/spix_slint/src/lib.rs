// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The Spix XML-RPC test server for a Slint app written in Rust: the twin of the C++ `Spix::Slint`
//! backend, so that one GUI test suite drives a Qt app and a Slint app unchanged.
//!
//! The protocol is `spix_server`'s; this crate is its Slint backend. [`start`] takes the windows
//! to serve by name, and the calls are handed to the GUI thread with
//! `slint::invoke_from_event_loop`. A path is `<window>/<id>/<id>/...`: the first segment names a
//! registered window, and each following one is an `accessible-id` looked for among the
//! descendants of the previous match. What Slint's element search does and doesn't show is written
//! down in the spix repository's `PROGRESS.md`.

mod backend;
mod input;
mod scene;

#[cfg(test)]
mod test_ui;

use slint::{ComponentHandle, Weak};

pub use spix_server::Error;

/// Starts a Spix server on `127.0.0.1:port`, serving `windows` under their names: the first
/// segment of every path. Call it on the GUI thread, with the windows made and before the event
/// loop runs; the calls it gets are handed to that thread.
///
/// # Errors
///
/// If nothing can listen on `port`, or the thread that answers the calls can't be started.
pub fn start<'a, T: ComponentHandle + 'static>(
    port: u16,
    windows: impl IntoIterator<Item = (&'a str, Weak<T>)>,
) -> Result<(), Error> {
    for (name, window) in windows {
        backend::register(name, window);
    }
    spix_server::start(backend::SlintBackend, port)
}
