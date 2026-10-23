# spix_slint

The Spix XML-RPC test server for a Slint app written in Rust: the twin of Spix's C++
`Spix::Slint` backend, so that one GUI test suite drives a Qt app and a Slint app unchanged.
The protocol is the one in `docs/rpc-api.md`.

```rust
let window = MainWindow::new()?;
spix_slint::start(9000, [("mainWindow", window.as_weak())])?;
window.run()?;
```

`start` listens on `127.0.0.1:<port>` and answers these calls: clicks, key and
text input, `existsAndVisible`, get and set of `text`, `checked`, `enabled` and `count`,
`getBoundingBox`, `wait`, `quit` and `getErrors`. Anything else is a fault reading `unsupported`.

- **Paths** are `<window>/<id>/<id>/...`: the first segment names a window registered with
  `start`, and every following one is an `accessible-id` looked for depth-first among the
  descendants of the previous match, the first one in tree order winning. The window itself has no
  element behind it: Slint's search never visits it.
- **Threading**: the listener thread hands each call to the GUI thread with
  `slint::invoke_from_event_loop`, where it runs as a future (clicks wait on Slint's timers), and
  waits for the answer; `wait` sleeps on the listener thread so
  the app keeps running, and `quit` tells the event loop to stop from wherever it is called, so the
  app exits normally.
- **The app runs headless** with Slint's testing backend (`i-slint-backend-testing`), set before
  the window is created. Its findings, and the C++ backend this ports, are in this repository's
  `PROGRESS.md`.

Test with `cargo test`: the unit tests drive a small window of their own, and `tests/server.rs`
answers the whole protocol over a socket, `quit` included.
