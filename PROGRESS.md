# Slint backend for Spix

Goal: a Slint scene backend (`Spix::Slint`) built on Slint's C++ testing API (`slint-testing.h`), so GUI tests written in plain C++ against `spix::TestServer` run unchanged on a Qt app and on its Slint port.

Agents working on this: tick steps as you finish them, and add anything worth knowing to [Findings and for later](#findings-and-for-later). Work on the `slint` branch, through git-loom.

## Background

- `slint-testing.h` needs `SLINT_FEATURE_EXPERIMENTAL=ON`, so Slint must be built from source. It's a submodule in `3rdparty/slint`, pinned to v1.18.0, which has everything needed. A local clone with full history is at `/pub_data/sources/slint`.
- The C++ `ElementHandle` API can visit elements and find them by accessible label, element id or type name, and read `accessible_*` properties, `size()` and `absolute_position()`. It has no parent/children navigation, and it skips elements with `visible: false`.
- Input goes through `window().dispatch_pointer_press/release/move_event()` and `dispatch_key_press/release_event()`. Pointer events carry no modifiers.
- `slint::testing::init()` mocks time, which would freeze the bot's timer and Spix `wait()`. Don't use it. Run a real backend.
- Elements are identified by `accessible-id`, which requires a constant `accessible-role` on the element.

## Steps

### Phase 0: build setup
- [ ] CMake option `SPIX_BUILD_SLINT` (default OFF), `libs/Scenes/Slint/` skeleton
- [ ] Slint as a git submodule in `3rdparty/slint`, pinned to `v1.18.0`, `shallow = true` in `.gitmodules`. Locally, init with `--reference /pub_data/sources/slint` to reuse that clone's objects
- [ ] `add_subdirectory(3rdparty/slint/api/cpp)` when `SPIX_BUILD_SLINT` is ON, forcing `SLINT_FEATURE_EXPERIMENTAL` and `SLINT_FEATURE_TESTING` ON before it
- [ ] `dev-slint` preset inheriting `dev`, with `SPIX_BUILD_SLINT=ON`
- [ ] Add `stb` to `vcpkg.json` (PNG writing for screenshots)
- [ ] Check whether `SLINT_BACKEND=testing` works in a C++ build
- [ ] Check whether `SLINT_FEATURE_MCP=ON` makes `SLINT_BACKEND=headless` available. Decide how CI runs without a display (`headless` or `xvfb-run`)

### Phase 1: `Spix::Slint` library
- [ ] `SlintBot`: `addWindow(name, ComponentHandle<T>)` (type-erased), `runTestServer()`, `slint::Timer` every 10 ms calling `CommandExecuter::processCommands()`
- [ ] Test-helper hooks on `SlintBot`: property getters/setters and method handlers for names Slint can't resolve itself
- [ ] `SlintScene::itemAtPath`: window by registered name, path names matched against `accessible-id`, nesting checked by geometry (each match inside the previous one's rect), first match in tree order wins
- [ ] Path selectors: `#Type` via `type_name()`/`bases()`, `"text"` value and `(prop=value)` via the property mapping. `.property` unsupported (report an error)
- [ ] `SlintItem`: `size()`, `position()` (window position + `absolute_position()`, matching Qt's screen coordinates), `bounds()`, `visible()`
- [ ] `SlintItem::stringProperty`: `text`, `checked`, `enabled`, `value`, `description`, `count`, `x`/`y`/`width`/`height`, raw `accessible.<name>`, then the registered getters
- [ ] `SlintItem::setStringProperty` (`text`/`value` via `set_accessible_value`, `checked` via default action) and `invokeMethod` (`click`, `increment`, `decrement`, `expand`, then registered handlers)
- [ ] `SlintEvents`: mouse move/press/release, button mapping, modifiers as key presses around the click, Spix `KeyCodes` → `slint::platform::key_codes`, `stringInput`, `quit()`. `extMouseDrop` reports unsupported
- [ ] Screenshots: `take_snapshot()`, crop to the element using the scale factor, PNG via stb, base64

### Phase 2: examples (`examples/slint/`)
- [ ] Move the `examples/qtquick/Basic` test body to `examples/shared/BasicTests.h`. The Qt example still passes
- [ ] `examples/slint/Basic`: same UI (`Button_1`, `Button_2` with left/right click, `results` text), using the shared test
- [ ] `examples/slint/GTest`: port of `examples/qtquick/GTest`, registered with ctest

### Phase 3: unit tests (`libs/Scenes/Slint/tests/`)
- [ ] Path lookup: by `accessible-id`, nesting, ids in repeaters, hidden elements
- [ ] Property mapping
- [ ] Clicks reaching a `TouchArea`, key input into a `LineEdit`
- [ ] Screenshot of an element

### Phase 4: CI
- [ ] CI job building with `SPIX_BUILD_SLINT=ON` and running the Slint tests and examples. Checkout with `submodules: recursive`, Rust toolchain installed, cargo build cache

# Findings and for later

Open questions, limitations and things worth revisiting. Append here when you find something.

- **`visible` on hidden elements.** Slint's element search skips `visible: false` elements, so `getStringProperty(path, "visible")` reports "item not found" instead of returning `"false"`. Not a blocker for new tests: use `existsAndVisible()`. Matters when porting existing Qt tests unchanged (pointless `test_gui` checks `editTask` and `weekNavigator` for `"false"`). Options: change those tests, or make Spix Core's `GetProperty` return `"false"` for a missing element when the property is `visible`.
- **No parent/children navigation in the C++ testing API.** Nested paths rely on geometric containment, which breaks when a child draws outside its parent. Rust already has `query_descendants()`. An upstream Slint patch exposing it (or a parent accessor) through `ffi.rs` and `slint-testing.h` would make path lookup exact. Decide whether to send it.
- **Custom properties** (`isActive`, `taskTagName`, ...) can't be read by name in compiled Slint. Only accessible properties and geometry are available. They need the test-helper hooks, or tests restricted to accessible properties. Design this when porting pointless.
- **Popups:** unknown whether `PopupWindow` contents are visited when searching from the root component.
- **`accessible-id` needs a constant `accessible-role`.** Every element a test looks up must have one (documented in qt_to_slint MAP.md, Item entry).
- **Submodule URL.** Upstream `slint-ui/slint` is enough while we only consume Slint. If we carry patches (e.g. parent/children navigation) before they land upstream, point the submodule at a fork, as is done for `3rdparty/vcpkg`.
- **Experimental API.** `slint-testing.h` has no compatibility guarantees. Keep Slint pinned and expect breakage on upgrades.
