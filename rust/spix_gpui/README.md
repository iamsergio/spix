# spix_gpui

The Spix XML-RPC test server for a GPUI app written in Rust, the twin of `spix_slint`. The protocol
is the one in `docs/rpc-api.md`, and lives in `../spix_server`, the toolkit-neutral server this
crate is a backend of: its codec, listener, faults, `getErrors`, gestures and screenshot cropping
are described there. What stays here is what is GPUI.

GPUI comes through `gpui-kit` (0.9.1, pinned exactly, with its `test-support` feature), which
re-exports the `gpui-pre` snapshot of GPUI.

```rust
gpui_kit::platform::application().run(|cx| {
    gpui_kit::init(cx);
    let (window, view) = gpui_kit::open_window(options, cx, |window, cx| { /* ... */ })?;
    spix_gpui::start(9000, [("mainWindow", window)], cx)?;
});
```

## API

```rust
pub fn start<'a, W: Into<AnyWindowHandle>>(
    port: u16, windows: impl IntoIterator<Item = (&'a str, W)>, cx: &mut App,
) -> Result<(), Error>;
pub fn register_input(path: &str, state: Entity<InputState>);
pub fn register_textarea(path: &str, state: Entity<TextareaState>);
pub fn on_property(hook: impl Fn(&Window, &mut App, &[String], &str) -> Option<String> + 'static);
pub fn on_set_property(
    hook: impl Fn(&mut Window, &mut App, &[String], &str, &str) -> bool + 'static,
);
```

All five are called on GPUI's foreground thread, from the app's setup: what they remember is
thread-local, since GPUI's handles are not `Send`.

## Calls answered

Everything `spix_server` serves: `mouseClick`, `mouseClickWithButton`, `mouseDoubleClick`,
`mouseWheel`, `mouseDrag`, `inputText`, `enterKey`, `existsAndVisible`, `getStringProperty`,
`setStringProperty`, `getBoundingBox`, `takeScreenshot`, `getErrors`, `wait` and `quit`. Anything
else, `takeScreenshotAsBase64` included, is a fault reading `unsupported`.

## Paths

A path is `<window>/<id>/<id>/...`. The first segment names a window registered with `start`; with
no further segment the path is the window itself. The others are looked up in the registry that
the `test-support` feature of GPUI Kit fills while a frame paints, where each element is known by
its full path: the chain of element ids from the root of the window down to it, in which only
ancestors that have an id appear and components add type-name segments of their own.

So a path can't be matched segment by segment. The rule is: the last segment is the element's own
id, and the segments before it are among the named segments of its ancestors, in that order, with
anything in between (a subsequence). `mainWindow/composer/send` finds an element with the id `send`
that has an ancestor with the id `composer`, however many others lie between. An id is a name
(`.id("send")`) or a name and a number (`.id(("row", 3))`, spelled `row-3`).

Several elements can match. The first is: a visible one before an invisible one, then top to
bottom and then left to right by the top-left corner of the bounds, then the outer one before the
one inside it. The registry has no order of its own. Two lists with a `row-0` each are told apart
by a path (`mainWindow/left/row-0`).

A virtual list registers only the rows it paints, so a row out of view does not exist until the
list is scrolled to it. Elements in a popup (a menu, a popover) register under a root of their own
and are found by the same rule, and clicked where they are drawn.

## What the app must do

- **Give elements ids**, and call `.test_support()` on every element it makes itself: GPUI Kit's
  components (buttons, inputs, checkboxes...) register themselves, a plain `div().id("x")` does
  not. Without the `test-support` feature of `gpui-kit` the registry is empty and nothing is found.
- **Say what a label says**: `.aria_label(text)` next to `.child(text)` on every element whose
  `text` a test reads. The registry does not record the text a `div` draws. `text` is an input's
  value when the element has one, and its label otherwise.
- **Register inputs** with `register_input("roomComposer/composerField", entity)` (`register_textarea` for a multi-line `TextareaState`, the same otherwise), so that
  `setStringProperty(path, "text", ...)` can set the value (`InputState::set_value`) and
  `getStringProperty(path, "text")` read it (`InputState::value`, so a masked input answers its
  real text; the registry exposes none for it). The registry can't write an input. The path is a
  Spix path below the window, as tests spell it, or just the id when it is unique; inputs may share
  an id in different scopes. An element takes the registered input whose path matches its own by
  the rule above. With none the call goes to
  `on_set_property`; with more than one nothing is set and the call is an error in `getErrors`
  (the generic "not settable" one, as the server words it; the clashing paths go to stderr).
  Registering a path again replaces its input. Reading `text` with none or several
  matching inputs answers from the registry.
- **Answer the rest with hooks**: `checked` (`true`/`false`) and `enabled` come from the registry,
  as the `checked` and `disabled` an element reports. Anything else a test asks for, `count` of a
  list from its model for one, goes to the property hook `on_property`, which gets the path of
  the element (the named segments of its registry path from the root down, as a Spix path spells
  them, its own id last, so an app can match its own scopes; just the window's name for the window)
  and the property, and answers `None` for what it
  doesn't know, which the client reads as the empty string. The hook runs inside an update of the
  window and gets it, to read the registry or shape text with: it may read entities, not update
  that window. Setting any property other than an input's `text` goes to `on_set_property`, with the same path; without one, or when it answers `false`, the call is an error
  in `getErrors`.

## Threading

`spawn_on_gui` sends each call over a channel to a loop that `start` spawned on GPUI's foreground
executor, which spawns the call's future on it and polls it between frames; the listener waits for
the answer. `sleep` is a timer of GPUI's executor. `quit` hops the same way and calls
`App::quit`, so the app's `run` returns.

Input is GPUI's own platform events (`Window::dispatch_event`), so hit testing, focus, hover and
drag run as for a user, and a frame is drawn after each. The modifiers held for a click are the
window's own (a modifiers-changed event is sent) and ride on every pointer event. A double click
is two presses with `click_count` 1 and 2. A wheel notch (120 in Qt's units) is 60 logical pixels,
as for Slint; the sign is the same in both toolkits. `enterKey` maps Qt's key codes to GPUI's key
names (Return, Tab, arrows, F1 to F24, printable ASCII...) and drops a code without a key.

## Screenshots

`takeScreenshot` asks the X server for the pixels of the window (`GetImage`, through `x11rb`, no
`unsafe`), because GPUI can't read back a Linux window itself (`Window::render_to_image` is not
implemented for them). The window's X id comes from the raw window handle GPUI gives out, and the
display is the one `$DISPLAY` names. Consequences:

- **X11 only.** On Wayland, or with GPUI's headless platform (which `application()` picks when no
  display is set, and which draws nothing), there are no pixels: the call reports the reason in
  `getErrors`, as for any renderer without pixels.
- The image is physical pixels, with `Window::scale_factor()` for the crop of an element.
- It shows what the compositor shows: the window must be mapped, and not covered by another one
  on a server without compositing.

## Tests

`cargo test` runs the unit tests (path and key mapping, and the backend against a window of its
own with the elements of `tests/support/mod.rs`, a popup menu opened by a right click included) and
`tests/server.rs`, which speaks the whole protocol over a socket and ends with `quit`. GPUI wants
one app per process, so each of the two binaries starts one.

- With a display: `DISPLAY=:1 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json cargo
  test` (lavapipe, for a machine without a GPU; the file's name varies, and `xvfb-run -a cargo
  test` does for the display). Everything runs, the screenshot included.
- Without one: `env -u DISPLAY -u WAYLAND_DISPLAY cargo test`. GPUI uses its headless platform.
  Everything but the pixels runs, and the screenshot checks are reduced to the error the call
  reports, with a message saying so.
- Packages needed on Debian: `libxkbcommon-x11-dev`, `libxcb-xkb-dev`, plus `libvulkan1` and
  `mesa-vulkan-drivers` at run time (and `xvfb`).
