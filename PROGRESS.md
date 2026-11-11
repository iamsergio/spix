# Slint backend for Spix

Goal: a Slint scene backend (`Spix::Slint`) built on Slint's C++ testing API (`slint-testing.h`), so GUI tests written in plain C++ against `spix::TestServer` run unchanged on a Qt app and on its Slint port.

Agents working on this: tick steps as you finish them, and add anything worth knowing to [Findings and for later](#findings-and-for-later). Work on the `slint` branch, through git-loom.

## Background

- `slint-testing.h` needs `SLINT_FEATURE_EXPERIMENTAL=ON`, so Slint must be built from source (v1.18.0 has everything needed). Spix finds it with `find_package(Slint)`, or uses the `Slint::Slint` target of a parent project.
- The C++ `ElementHandle` API can visit elements and find them by accessible label, element id or type name, and read `accessible_*` properties, `size()` and `absolute_position()`. It has no parent/children navigation, and it skips elements with `visible: false`.
- Input goes through `window().dispatch_pointer_press/release/move_event()` and `dispatch_key_press/release_event()`. Pointer events carry no modifiers.
- `slint::testing::init()` mocks time, which would freeze the bot's timer and Spix `wait()`. Don't use it. Run a real backend.
- Elements are identified by `accessible-id`, which requires a constant `accessible-role` on the element.

## Steps

### Phase 0: build setup
- [x] CMake option `SPIX_BUILD_SLINT` (default OFF), `libs/Scenes/Slint/` skeleton
- [x] `find_package(Slint)` when `SPIX_BUILD_SLINT` is ON, checking it has `EXPERIMENTAL`, `TESTING` and `SYSTEM_TESTING`
- [x] `dev-slint` preset inheriting `dev`, with `SPIX_BUILD_SLINT=ON`
- [x] Add `stb` to `vcpkg.json` (PNG writing for screenshots)
- [x] Check whether `SLINT_BACKEND=testing` works in a C++ build
- [x] Check whether `SLINT_FEATURE_MCP=ON` makes `SLINT_BACKEND=headless` available. Decide how CI runs without a display (`headless` or `xvfb-run`)

### Phase 1: `Spix::Slint` library
- [x] `SlintBot`: `addWindow(name, ComponentHandle<T>)` (type-erased), `runTestServer()`, `slint::Timer` every 10 ms calling `CommandExecuter::processCommands()`
- [x] Test-helper hooks on `SlintBot`: property getters/setters and method handlers for names Slint can't resolve itself
- [x] `SlintScene::itemAtPath`: window by registered name, path names matched against `accessible-id`, nesting checked by geometry (each match inside the previous one's rect), first match in tree order wins
- [x] Path selectors: `#Type` via `type_name()`/`bases()`, `"text"` value and `(prop=value)` via the property mapping. `.property` unsupported (report an error)
- [x] `SlintItem`: `size()`, `position()` (window position + `absolute_position()`, matching Qt's screen coordinates), `bounds()`, `visible()`
- [x] `SlintItem::stringProperty`: `text`, `checked`, `enabled`, `value`, `description`, `count`, `x`/`y`/`width`/`height`, raw `accessible.<name>`, then the registered getters
- [x] `SlintItem::setStringProperty` (`text`/`value` via `set_accessible_value`, `checked` via default action) and `invokeMethod` (`click`, `increment`, `decrement`, `expand`, then registered handlers)
- [x] `SlintEvents`: mouse move/press/release, button mapping, modifiers as key presses around the click, Spix `KeyCodes` → `slint::platform::key_codes`, `stringInput`, `quit()`. `extMouseDrop` reports unsupported
- [x] Screenshots: `take_snapshot()`, crop to the element using the scale factor, PNG via stb, base64

### Phase 2: examples (`examples/slint/`)
- [x] Move the `examples/qtquick/Basic` test body to `examples/shared/BasicTests.h`. The Qt example still passes
- [x] `examples/slint/Basic`: same UI (`Button_1`, `Button_2` with left/right click, `results` text), using the shared test
- [x] `examples/slint/GTest`: port of `examples/qtquick/GTest`, registered with ctest

### Phase 3: unit tests (`libs/Scenes/Slint/tests/`)
- [x] Path lookup: by `accessible-id`, nesting, ids in repeaters, hidden elements
- [x] Property mapping
- [x] Clicks reaching a `TouchArea`, key input into a `LineEdit`
- [x] Screenshot of an element

### Phase 4: CI
- [x] CI job building with `SPIX_BUILD_SLINT=ON` and running the Slint tests and examples. Builds Slint v1.18.0 from source first and caches its install

### Phase 5: `spix_slint`, the server in Rust
Goal: the same protocol for a Slint app written in Rust, where this backend can't be used at all:
it needs C++-generated Slint components, and a Rust Slint app has a different Slint runtime. Ports
the *logic* of Phase 1, in `rust/spix_slint/`, so that a GUI test suite written against Spix drives
a Qt app and its Rust Slint port unchanged.

- [x] The listener thread: `std::net`, the little HTTP/1.1 that `ureq` sends, `roxmltree` decoding
  of the calls in `docs/rpc-api.md` that a GUI test suite needs. Every method but `wait` and `quit` is handed to the
  GUI thread with `slint::invoke_from_event_loop` and waits for the answer there; `wait` sleeps on
  the listener thread so the app keeps running, `quit` calls `slint::quit_event_loop()` from
  wherever it is called, so `main` returns normally
- [x] Paths `<window>/<id>/<id>/...`: the windows come from
  `spix_slint::start(port, [("mainWindow", window.as_weak())])`, and each following segment is an
  `accessible-id` looked for with `visit_descendants` among the descendants of the previous match,
  the first in tree order winning. Real ancestry instead of Phase 1's geometric containment; the
  window itself still has no element behind it
- [x] Properties as `SlintItem` maps them (`text` from accessible value or label, `checked`,
  `enabled`, `count`), bounds in screen coordinates from `absolute_position()` plus the window's
  place on the screen, and input: clicks through Slint's own `ElementHandle::single_click` and
  `double_click` (which know where a popup item stands), modifiers as key presses around them,
  `inputText`/`enterKey` dispatched to the window, Spix `KeyCodes` → `slint::platform::Key`
- [x] Faults for everything outside the protocol (fault code -32601, `unsupported`), and the error
  strings of `getErrors` as C++ Spix words them, since clients retry on them
- [x] Tests: unit tests on a window of its own (`slint::slint!`) for decode/encode, faults, path
  lookup, properties and text input; one test binary for the whole protocol over a socket — clicks,
  a menu opened by a click and by a right click, `takeScreenshot`, and `quit`
- [x] `takeScreenshot(path, file)`: `Window::take_snapshot()` cropped to the element's window
  rectangle times the scale factor, clamped, as `SlintScene::screenshotPng` does; PNG by the `png`
  crate, written before the call answers. A missing path, or a renderer without pixels, goes into
  `getErrors`. `takeScreenshotAsBase64` is not implemented

### Phase 6: a toolkit-neutral Rust server
Goal: a second Rust backend (another GUI toolkit, perhaps a C library) should reuse everything of
`spix_slint` that isn't Slint. The crate is split into `rust/spix_server`, which is the protocol,
and `spix_slint`, a backend on top of it with its public API unchanged (`spix_slint::start` and
`spix_slint::Error`). No behaviour change.

- [x] `rust/spix_server`: the XML-RPC codec, HTTP listener, faults, `getErrors`, parameters, `wait`,
  path splitting, the gesture choreography (click, double click, modifiers, wheel, drag) and the
  screenshot cropping and PNG encoding, behind a `Backend` trait of toolkit primitives
- [x] `spix_slint` implements `Backend`: element search, properties (and `frameCount`, `truncated`),
  pointer and key events, a sleep on Slint's timers, `take_snapshot()`, and the click primitive
  overridden with `single_click`/`double_click` so that popup items are still hit
- [x] The tests moved with the code: the protocol tests run in `spix_server` over a small fake
  backend (new ones for the click, modifier, wheel and drag event sequences, and for cropping), the
  scene and input tests and the socket test stay in `spix_slint`
- [x] Both crates are self-contained (no `workspace = true`), with the same `[lints]` written out,
  and `spix_slint` depends on `spix_server` by path
- [x] `.github/workflows/rust.yml`: format, clippy `-D warnings` and tests of each crate under
  `rust/`, on 1.98.1

### Findings of the split (Phase 6)

- **The seam is one trait, shared with the listener thread.** `Backend` is `Send + Sync`, so the
  server keeps the `getErrors` log itself and a backend holds only what is the toolkit's; the
  thread-bound parts (windows, timers) stay in the backend's own thread-locals, as Slint's did.
  The one thing that crosses threads is a `GuiTask`: a `Send` closure that makes a non-`Send`
  future once it is on the GUI thread, which is what a toolkit with its own executor (Slint's
  `spawn_local`, GPUI's foreground executor) can run.
- **Item identity is the backend's.** `find` returns an opaque `Item`, and every primitive takes
  the window name next to it, so a backend can look its window up again and report a window that
  went away between two events of a gesture. The window itself is the item of a path without
  segments; the server, not the backend, knows to refuse a click on it.
- **Positions are window-logical, bounds are screen-logical.** A drag or a wheel turn needs the
  item's place in the window (`frame`); `getBoundingBox` adds where the window is on the screen
  (`bounds`); a screenshot crops the frame times the image's scale factor. Slint produced the
  same three numbers from one geometry function.
- **The click is a primitive with a default.** A toolkit that can tell where an item stands better
  than the centre of its frame (an item in a popup, in Slint's case) overrides it; the default
  presses and releases at the centre. Modifiers are held around it by the server.
- **`wait`, `quit` and the error log don't need the GUI thread**, and still don't touch it.
- **Error wording that names the toolkit** (the one for a property that can't be set) comes from a
  `TOOLKIT` constant of the backend, so the Slint wording is unchanged.
- **`async fn` in the trait needs `#[allow(async_fn_in_trait)]`**: the futures are deliberately not
  `Send`, since they only ever run on the GUI thread.
- **No toolkit-neutral screenshot.** Some toolkits have no in-process pixels (grabbing the X11
  window is the fallback): `window_image` returns an error string for those, which is what a mock
  renderer already did.

### Phase 7: a GPUI backend
Goal: `rust/spix_gpui`, a second backend of `spix_server`, so that a GUI test suite written against
Spix also drives an app written with GPUI (through `gpui-kit` 0.9.1, which re-exports the
`gpui-pre` 0.3.12 snapshot of GPUI). `spix_server`'s API did not change.

- [x] Lookup in the registry that GPUI Kit's `test-support` feature fills while a frame paints
  (`.test_support()` on an element with an id; GPUI Kit's components register themselves), a path
  matched as a subsequence of the named segments of an element's full path
- [x] Properties: `text` (an input's value, else the `aria_label` the app gave), `checked`,
  `enabled`; anything else, and setting a property, through hooks the app registers, and
  `setStringProperty` of `text` on an input the app registered; `getStringProperty` of `text` on
  such an input answers its real value (a masked input's too), through the same path matching
- [x] Pointer, wheel, keys, typed text and modifiers as GPUI platform events through
  `Window::dispatch_event`, with a click count in the events for a double click
- [x] The hop onto GPUI's foreground thread (a channel and a `cx.spawn` loop), timers of GPUI's
  executor, `quit` as `App::quit`
- [x] `takeScreenshot` by a `GetImage` of the window's X id (`x11rb`, no `unsafe`); an error in
  `getErrors` where there are no pixels (Wayland, the headless platform)
- [x] Tests: unit tests against a window of their own (a popup menu opened by a right click
  included), and a socket test of the whole protocol, `quit` included; both pass with a display
  and without one (the screenshot is then checked for its error)
- [x] `.github/workflows/rust.yml`: a job for the crate with the system packages it links, run
  under `xvfb-run` with lavapipe

### Findings of the GPUI backend (Phase 7)

- **The registry is the only lookup that works in a live window.** AccessKit's tree is only built
  while an assistive technology is connected, and `debug_selector` is not readable from outside
  GPUI. The registry is thread-local, filled during paint and read on the foreground thread, so
  the backend draws a frame (`render_frame`) before every search and after every event: what a
  call reads is the result of the call before it.
- **The registry is unordered**, so "the first match" is made one: visible matches before
  invisible ones, then top to bottom and left to right by the corner of the bounds (the order of
  GPUI Kit's own `find_all`), then the outer element before the one inside it. An item is kept as
  its full path and read from the registry again for each question, since a list that scrolls
  moves it. `snapshots()` doesn't carry the semantics of a component's native editor (an input's
  value), `find_all` does, so a match is resolved through `find_all` by its exact path.
- **Popups register under a separate root** (`deferred`), with window coordinates like the rest, so
  they are found by the same rule and clicked at the centre of their frame: no override of the
  click's geometry was needed, only of its click count.
- **A virtual list registers the rows it paints.** A row out of view is not found, as with Slint's
  repeater; the list's row count is something only the app's model knows, so `count` is a hook.
- **A label has no text in the registry.** Text of a plain `div` is not recorded, so a label a test
  reads needs `.aria_label(text)` next to `.child(text)`, the duty Slint's `accessible-id` is.
- **Setting an input's text** can't go through the element: the app keeps the `Entity<InputState>`
  and registers it under a Spix path below the window. Ids are not unique across scopes, so
  inputs are matched to the element by the path rule, not by own id; two matches set nothing, and
  the hooks get the element's spelled path instead of its id for the same reason. The trait has
  no way to report a custom error, so an ambiguity is the server's generic "not settable" one,
  with the clashing paths on stderr. `InputState::set_value` emits no change event.
- **The wheel keeps its sign**: Qt's positive y (towards the start) and GPUI's pixel delta agree,
  so only the unit changes, 60 pixels to Qt's 120 units, as for Slint.
- **Screenshots are the weak spot.** `Window::render_to_image` is not implemented for GPUI's Linux
  windows, so the backend grabs the window from the X server, from the id in the raw window handle
  (`RawWindowHandle::Xcb`). X11 only, on the display `$DISPLAY` names, 32-bit little-endian pixels.
  GPUI's headless platform (what `application()` falls back to without a display) has no native
  handle and draws nothing.
- **One GPUI app per process**, and it runs wherever `run` is called, a test thread included, so
  each test binary starts one app and runs its checks in it; a failed check returns its message
  instead of panicking inside GPUI's executor.
- **`async fn` with nothing to await** is all the click needs in this trait: the whole gesture is
  synchronous events, the awaits are the server's.

# Findings and for later

Open questions, limitations and things worth revisiting. Append here when you find something.

- **`visible` on hidden elements.** Slint's element search skips `visible: false` elements, so `getStringProperty(path, "visible")` reports "item not found" instead of returning `"false"`. Not a blocker for new tests: use `existsAndVisible()`. Matters when porting existing Qt tests unchanged (pointless `test_gui` checks `editTask` and `weekNavigator` for `"false"`). Options: change those tests, or make Spix Core's `GetProperty` return `"false"` for a missing element when the property is `visible`.
- **No parent/children navigation in the C++ testing API.** Nested paths rely on geometric containment, which breaks when a child draws outside its parent. Rust already has `query_descendants()`. An upstream Slint patch exposing it (or a parent accessor) through `ffi.rs` and `slint-testing.h` would make path lookup exact. Decide whether to send it.
- **Custom properties** (`isActive`, `taskTagName`, ...) can't be read by name in compiled Slint. Only accessible properties and geometry are available. They need the test-helper hooks, or tests restricted to accessible properties. Design this when porting pointless.
- **Popups:** unknown whether `PopupWindow` contents are visited when searching from the root component.
- **`accessible-id` needs a constant `accessible-role`.** Every element a test looks up must have one (documented in qt_to_slint MAP.md, Item entry).
- **Slint fork.** Upstream `slint-ui/slint` is enough while we only consume Slint. If we carry patches (e.g. parent/children navigation) before they land upstream, CI should build a fork.
- **Experimental API.** `slint-testing.h` has no compatibility guarantees. Keep Slint pinned and expect breakage on upgrades.
- **Slint is linked as a shared library, Spix stays static.** Slint's CMake declares `BUILD_SHARED_LIBS=ON`, which would turn all Spix libs into shared ones too and break `SpixCoreTests` (it uses hidden symbols). A parent project adding Slint with `add_subdirectory()` should set it ON only around that call. Static Slint was tried: executables fail to link (undefined `Fc*`, fontconfig isn't listed as a dependency), so shared is also the less fragile choice. Examples/tests need `libslint_cpp.so` on the rpath (CMake handles it in the build tree).
- **`ElementHandle` needs debug info in the generated code.** Without it the search returns 0 elements and prints "requires the presence of debug info... SLINT_EMIT_DEBUG_INFO=1". `slint_target_sources()` only sets that when `SLINT_FEATURE_SYSTEM_TESTING` is ON, so Spix requires it (inert unless `SLINT_TEST_SERVER` is set at runtime). A ported app built outside this tree must either enable that feature or export `SLINT_EMIT_DEBUG_INFO=1` while building. Add to qt_to_slint PORT.md.
- **Slint features chosen** (`scripts/build_slint.sh` and CI): ON `EXPERIMENTAL`, `TESTING`, `SYSTEM_TESTING`, `MCP`. OFF `INTERPRETER`, `SYSTEM_TRAY`, `GETTEXT`. Kept default winit backend + femtovg + software renderer (software is what headless/take_snapshot uses).
- **`SLINT_BACKEND=testing` does not work from C++.** Slint prints "Could not load rendering backend testing, fallback to default" and uses winit. The selector's `backend-testing` cargo feature is not reachable from any `SLINT_FEATURE_*` option. Also moot, since we need real time (see Background).
- **`SLINT_FEATURE_MCP=ON` makes `SLINT_BACKEND=headless` work**, and `window().take_snapshot()` works under it (verified with a scratch program: found an element by accessible label, `size()` correct, snapshot pixels correct, `slint::Timer` and `quit_event_loop()` work with real time). With MCP, headless is also the last-resort fallback when no display is available, so no env var is even needed on CI; without MCP the same program aborts with "No backends configured" when `DISPLAY` is unset. MCP and SYSTEM_TESTING are env-gated (`SLINT_MCP_PORT`, `SLINT_TEST_SERVER`), so they're inert by default. Cost: roughly +1 minute of CPU time on a clean build, about the same wall time. `scripts/build_slint.sh` enables `SLINT_FEATURE_MCP=ON`.
- **CI recommendation:** use `SLINT_BACKEND=headless` (needs the MCP feature, no X server, no `xvfb-run`; neither Xvfb nor xvfb-run is installed here anyway). Keep `xvfb-run` as a fallback only for tests that need real winit behaviour (window positions, real focus).
- **Snapshot size depends on the backend's scale factor.** On the dev machine's X display (scale 1.5) a 200x100 window gives a 300x150 snapshot, headless gives 200x100. Crop using `window().scale_factor()`, never assume 1.0.
- **Build times** (32 cores, Debug): clean `scripts/build_slint.sh` about 27 s wall (about 6 min CPU), incl. cargo build of Slint. CI runners are much slower, so CI caches the Slint install.
- **`SLINT_BACKEND=testing` from C++.** Upstream, the testing backend is only reachable through `i-slint-backend-selector`'s `backend-testing` feature, which enables `i-slint-backend-testing/internal` (meant for Slint's own tests). `api/cpp/Cargo.toml` and `SlintFeatures.cmake` don't forward it, so exposing it is a few lines. Not useful for Spix: it renders nothing and measures text with a fixed font size, so geometry differs from the real app. A more useful upstream change would be decoupling the `headless` backend from the `mcp` feature (today `cfg(all(feature = "mcp", supports_headless))` in `internal/backends/selector/lib.rs`), but enabling MCP costs little, so it's low priority.
- **`slint-testing.h` has no include guard** (no `#pragma once`), so including it twice in one translation unit fails with "redefinition of ElementHandle". `Spix/SlintBot.h` includes it, so Spix's own headers include `SlintBot.h` rather than the Slint header directly. Users must not include both in one file unless they only get it through `SlintBot.h`. Worth an upstream one-line fix.
- **Phase 1 verification** was a throwaway program (not committed, Phase 3 adds real tests) under `SLINT_BACKEND=headless`: path lookup by `accessible-id`, `#Type`, `"text"` selectors, click (left/right) through a `TouchArea`, `inputText` with a non-ASCII string, `enterKey` with Shift, `setStringProperty` on `text` and `checked`, `existsAndVisible` false for `visible: false`, and a cropped PNG screenshot all behaved as expected. Not yet exercised: modifier+click, `(prop=value)` selectors, nested paths, repeaters, test-helper hooks, `mouseBeginDrag/EndDrag`.
- **`x`/`y` are window coordinates.** Qt reports them relative to the parent. Without parent navigation we only know the absolute position.
- **`text` mapping:** `accessible-value` if the element has one (text inputs), else `accessible-label` (Text, Button). A Slint element must set `accessible-label` (or have it default from its `text`) to be found by a `"text"` selector.
- **`SlintBot` keeps a strong reference to each added component** until `removeWindow()`.
- **Phase 2 verification:** the shared `BasicTests` body gives identical results on the Qt Basic example (run with `QT_QPA_PLATFORM=offscreen`) and the Slint one. `SpixSlintBasicExample` and `SpixSlintGTestExample` are registered with ctest (run with `SLINT_BACKEND=headless`) and pass. The Slint Basic UI also has the shift/control click variants, so the GTest example reuses its `main.slint`.
- **`GTest` port drops the property-selector clicks** (`.propertyWithTarget`, `.propertyWithParent`), see the unsupported `.property` selectors above.
- **`enable_testing()` moved** before `add_subdirectory(examples)` in the top-level CMake, otherwise examples can't register ctest tests.
- **Shared test timing:** `BasicTests` waits 100 ms between clicks instead of the 500 ms the Qt example used, to keep ctest fast.
- **Slint's element search doesn't visit the window's own element.** The first visited element is the window's first child. A window path therefore has no `ElementHandle`: `SlintItem` takes size and position from the window, supports only `width`/`height`/`x`/`y`/`visible` plus registered getters, and calls registered handlers with the window's first element. Found by the Phase 3 tests (Phase 1 had treated the first element as the root).
- **Phase 3 coverage** (`SpixSlintTests`, 27 tests, run headless): lookup, nesting, repeaters, hidden, `#Type`, `(prop=value)`, unsupported `.property`, property mapping, setters, built-in methods, hooks, clicks with buttons/modifiers, key input, screenshots. Not covered: `mouseBeginDrag`/`mouseEndDrag`, popups, `PopupWindow` contents (still unknown).
- Warnings from the scene are de-duplicated, since a lookup tests every element.

### Findings of the Rust server (Phase 5)

- **The testing backend's default renderer is a mock with no pixels**: `take_snapshot()` fails on
  it. `i-slint-backend-testing`'s `renderer-software` feature adds `renderer_name` to
  `TestingBackendOptions`; `Some("software".into())` gives windows a real rasterizer, which also
  measures text with real fonts, unlike the mock's fixed metrics.

- **Click into a popup with Slint's own helpers, as a future.** `absolute_position()` of a menu item
  is its position *inside* the popup, so a hand-made centre click misses it; `single_click` and
  `double_click` are public and use the popup-aware `absolute_center()`, but they wait on a real
  timer and deadlock if awaited inside the `invoke_from_event_loop` closure — `spawn_local` the
  call instead and send the answer when the future finishes. The helpers that advance mock time
  (`mock_single_click`, `mock_drag`) die on a real-time backend with `The platform's clock is not
  monotonic!`, and the `internal` feature exposing `active_popups()` doesn't compile from the
  published crate, which `include_bytes!`s fonts that only exist in the Slint repository.
- **`getBoundingBox` of a popup item is still relative to the popup** (it reads
  `absolute_position()`). No test reads popup geometry.
- **`slint::slint!` reads `SLINT_EMIT_DEBUG_INFO` when it compiles**; without it the element search
  finds nothing. A `build.rs` printing `cargo:rustc-env=SLINT_EMIT_DEBUG_INFO=1` turns it on; a UI
  compiled through `slint-build` uses `with_debug_info(true)` instead.
- **The testing backend is set per thread**, and `init_no_event_loop()` panics when the thread
  already has one, while libtest hands a thread to one test after another: tests that set the
  platform themselves keep working. `init_integration_test_with_system_time()` is still once per
  process, because it also claims the global event-loop proxy.
- **The testing backend opens an 800x600 window**, whatever the `.slint` window says; read a
  window's size back instead of assuming it.
- **`clicked` is for the left button only**: a right click reaches the element through its
  `pointer-event`, which is where a menu opened by right-click has to come from.
- **`slint::quit_event_loop()` can be called from any thread**, so `quit` is answered by the
  listener thread, its answer goes out first, and `main` returns normally.
- **A path dependency of a workspace member joins that workspace** while it sits inside the
  workspace directory, and errors with "multiple workspace roots" if it carries its own
  `[workspace]` table. `rust/spix_slint` is a member of such a workspace when this repository is a
  submodule inside one, and its own root in a clone of this repository alone, which its manifest
  written out in full makes possible.
- **A fault outside the protocol is `-32601` / `unsupported`**, for a method or a parameter type the
  server doesn't speak, so a test that calls one fails loudly instead of silently doing nothing.
