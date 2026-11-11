# spix_slint

The Spix XML-RPC test server for a Slint app written in Rust: the twin of Spix's C++
`Spix::Slint` backend, so that one GUI test suite drives a Qt app and a Slint app unchanged.
The protocol is the one in `docs/rpc-api.md`, and lives in `../spix_server`, the toolkit-neutral
server this crate is a backend of: its codec, listener, faults, `getErrors`, gestures and
screenshot cropping are described there. What stays here is what is Slint.

```rust
let window = MainWindow::new()?;
spix_slint::start(9000, [("mainWindow", window.as_weak())])?;
window.run()?;
```

`start` listens on `127.0.0.1:<port>` and answers the calls of `docs/rpc-api.md` that `spix_server`
serves, plus the properties `truncated` and `frameCount` (see below). Anything else is a fault
reading `unsupported`.

- **Paths** are `<window>/<id>/<id>/...`: the first segment names a window registered with
  `start`, and every following one is an `accessible-id` looked for depth-first among the
  descendants of the previous match, the first one in tree order winning. The window itself has no
  element behind it: Slint's search never visits it.
- **`truncated`** is `true` when the element's `accessible-description` is exactly `truncated`, and
  `false` otherwise, also when it has no description. Slint's testing API exposes no preferred
  size and no elision, so an app sets the description itself where its tests need the answer, for
  example `accessible-description: self.preferred-width > self.width ? "truncated" : "";` on a
  `Text` with `overflow: elide`.
- **`frameCount`**, read from the window itself (`getStringProperty("mainWindow", "frameCount")`),
  is the number of frames the window has drawn: Spix counts `RenderingState::AfterRendering`. The
  first read starts the counting and returns `0`, so an app whose tests never ask pays nothing, and
  a caller works with the difference between two reads. Spix takes the window's rendering notifier
  for this (a window has one), so the property is absent, the empty string as for any unknown
  property, when the app has set its own notifier or the renderer has none: the testing backend's
  software renderer, for one. Use a GPU renderer (`SLINT_BACKEND=winit`) to count.
- **Threading**: `spawn_on_gui` hands each call to the GUI thread with
  `slint::invoke_from_event_loop`, where it runs as a future with `slint::spawn_local` (clicks wait
  on Slint's timers), and the listener waits for the answer; `quit` is `slint::quit_event_loop()`.
- **Clicks** go through Slint's own `ElementHandle::single_click` and `double_click`, which know
  where an element inside an open popup stands; the backend overrides the click primitive of
  `spix_server` for that. Everything else of a gesture is `spix_server`'s.
- **The app runs headless** with Slint's testing backend (`i-slint-backend-testing`), set before
  the window is created. Its findings, and the C++ backend this ports, are in this repository's
  `PROGRESS.md`.
- **`takeScreenshot` needs pixels.** It crops a PNG of the element from `Window::take_snapshot()`,
  which the testing backend's default mock renderer can't give (the call then reports an error in
  `getErrors`). Set the platform yourself with `TestingBackendOptions { renderer_name:
  Some("software".into()), .. }` (the `renderer-software` feature of `i-slint-backend-testing`,
  which this crate turns on) instead of `init_integration_test_with_system_time()`.
  `takeScreenshotAsBase64` is not implemented.

Test with `cargo test`: the unit tests drive a small window of their own, and `tests/server.rs`
answers the whole protocol over a socket, `quit` included.
