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
- [x] CMake option `SPIX_BUILD_SLINT` (default OFF), `libs/Scenes/Slint/` skeleton
- [x] Slint as a git submodule in `3rdparty/slint`, pinned to `v1.18.0`, `shallow = true` in `.gitmodules`. Locally, init with `--reference /pub_data/sources/slint` to reuse that clone's objects
- [x] `add_subdirectory(3rdparty/slint/api/cpp)` when `SPIX_BUILD_SLINT` is ON, forcing `SLINT_FEATURE_EXPERIMENTAL` and `SLINT_FEATURE_TESTING` ON before it
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
- **Slint is linked as a shared library, Spix stays static.** Slint's CMake declares `BUILD_SHARED_LIBS=ON`, which would turn all Spix libs into shared ones too and break `SpixCoreTests` (it uses hidden symbols). The top-level CMake sets it ON only around `add_subdirectory(3rdparty/slint/api/cpp)` and restores the user's value after. Static Slint was tried: executables fail to link (undefined `Fc*`, fontconfig isn't listed as a dependency), so shared is also the less fragile choice. Examples/tests need `libslint_cpp.so` on the rpath (CMake handles it in the build tree).
- **`ElementHandle` needs debug info in the generated code.** Without it the search returns 0 elements and prints "requires the presence of debug info... SLINT_EMIT_DEBUG_INFO=1". `slint_target_sources()` only sets that when `SLINT_FEATURE_SYSTEM_TESTING` is ON, so the top-level CMake forces it ON (inert unless `SLINT_TEST_SERVER` is set at runtime). A ported app built outside this tree must either enable that feature or export `SLINT_EMIT_DEBUG_INFO=1` while building. Add to qt_to_slint PORT.md.
- **Slint features chosen** (`SPIX_BUILD_SLINT=ON`): forced ON `EXPERIMENTAL`, `TESTING`, `SYSTEM_TESTING`. Turned off (defaults, overridable in cache) `INTERPRETER`, `SYSTEM_TRAY`, `GETTEXT`. Kept default winit backend + femtovg + software renderer (software is what headless/take_snapshot uses). Slint is added `EXCLUDE_FROM_ALL`, so only what Spix links is built.
- **`SLINT_BACKEND=testing` does not work from C++.** Slint prints "Could not load rendering backend testing, fallback to default" and uses winit. The selector's `backend-testing` cargo feature is not reachable from any `SLINT_FEATURE_*` option. Also moot, since we need real time (see Background).
- **`SLINT_FEATURE_MCP=ON` makes `SLINT_BACKEND=headless` work**, and `window().take_snapshot()` works under it (verified with a scratch program: found an element by accessible label, `size()` correct, snapshot pixels correct, `slint::Timer` and `quit_event_loop()` work with real time). With MCP, headless is also the last-resort fallback when no display is available, so no env var is even needed on CI; without MCP the same program aborts with "No backends configured" when `DISPLAY` is unset. MCP and SYSTEM_TESTING are env-gated (`SLINT_MCP_PORT`, `SLINT_TEST_SERVER`), so they're inert by default. Cost: roughly +1 minute of CPU time on a clean build, about the same wall time. `dev-slint` enables `SLINT_FEATURE_MCP=ON`.
- **CI recommendation:** use `SLINT_BACKEND=headless` (needs the MCP feature, no X server, no `xvfb-run`; neither Xvfb nor xvfb-run is installed here anyway). Keep `xvfb-run` as a fallback only for tests that need real winit behaviour (window positions, real focus).
- **Snapshot size depends on the backend's scale factor.** On the dev machine's X display (scale 1.5) a 200x100 window gives a 300x150 snapshot, headless gives 200x100. Crop using `window().scale_factor()`, never assume 1.0.
- **Submodule is dissociated.** `git submodule add --reference` leaves `objects/info/alternates` pointing at the reference clone, which breaks if that clone moves. After adding, `git repack -a -d` plus removing the alternates file made it standalone (102 MB in `.git/modules/3rdparty/slint`; fsck is clean). `scripts/build_slint.sh` does no git operations and expects the submodules to be checked out.
- **Build times** (32 cores, Debug): clean `scripts/build_slint.sh` about 27 s wall (about 6 min CPU), incl. cargo build of Slint. CI runners will be much slower, so cache cargo.
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
