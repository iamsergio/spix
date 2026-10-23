// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The Spix XML-RPC test server for a Slint app written in Rust: the twin of the C++ `Spix::Slint`
//! backend, so that one GUI test suite drives a Qt app and a Slint app unchanged.
//!
//! [`start`] takes the windows to serve by name and answers the Spix calls on a thread of its own,
//! handing each of them to the GUI thread with `slint::invoke_from_event_loop`. A path is
//! `<window>/<id>/<id>/...`: the first segment names a registered window, and each following one
//! is an `accessible-id` looked for among the descendants of the previous match. What Slint's
//! element search does and doesn't show is written down in the spix repository's `PROGRESS.md`.

mod input;
mod scene;
mod server;
mod xmlrpc;

#[cfg(test)]
mod test_ui;

pub use server::{Error, start};
