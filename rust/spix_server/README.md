# spix_server

The toolkit-neutral half of the Spix XML-RPC test server, for GUI toolkits driven from Rust. It
holds everything that is the protocol in `docs/rpc-api.md` and nothing that is a toolkit; a
backend crate implements [`Backend`] for its toolkit and calls [`start`]. `spix_slint` is the
backend for Slint, and `spix_gpui` the one for GPUI.

```rust
spix_server::start(MyBackend::new(), 9000)?;
```

`start` listens on `127.0.0.1:<port>` and answers the calls of the protocol on a thread of its own.

## What the server owns

- The XML-RPC codec, the little HTTP/1.1 a client such as `ureq` sends, and the faults: `-32601`
  `unsupported` for a method or a parameter type outside the protocol, `-32600` for a request that
  isn't a call.
- The `getErrors` log, with the wording of C++ Spix (`<Command>: Item not found: <path>`): clients
  report these strings and may retry on them.
- The parameters of each call, and `wait`, which sleeps on the listener thread so that the app
  keeps running while it waits.
- Paths, `<window>/<id>/<id>/...`: the first segment names a window, the rest are ids.
- The choreography of the gestures, as primitive events and pauses:
  - a click presses and releases a button, with the modifier keys held down around it; a double
    click is two of them;
  - `mouseWheel` turns the wheel `steps` times over the centre of the item, `interval` apart;
  - `mouseDrag` presses at the start point, moves in `steps` equal parts, holds for `hold` and
    releases. Move `k` is due `k * interval` after the press and the release `steps * interval +
    hold` after it, so a late timer delays only its own event.
- `takeScreenshot`: the element's frame times the image's scale factor, clamped to the image,
  cropped out of an RGBA image of the window and encoded as a PNG.

## The backend

A [`Backend`] is `Send + Sync`, and shared with the listener thread, but all its methods except
`spawn_on_gui` and `quit` run on the GUI thread, where the toolkit's windows live.

| Method | What it is |
| --- | --- |
| `type Item`, `const TOOLKIT` | What a path finds (cloned across awaits, never leaves the GUI thread), and the toolkit's name for an error message |
| `spawn_on_gui(GuiTask) -> Result<(), GuiGone>` | Hops onto the GUI thread: runs the closure there and polls the future it makes there with an executor whose timers fire |
| `quit()` | Ends the event loop, from any thread |
| `find(window, names) -> Option<Item>` | The item under `names`; no names is the window itself |
| `frame(window, item) -> Option<Rect>` | Logical pixels from the window's top-left: where pointer events land and what a screenshot crops |
| `bounds(window, item) -> Option<Rect>` | Screen coordinates, for `getBoundingBox` |
| `visible(window, item) -> bool` | |
| `property(window, item, name) -> Option<String>` | `text`, `checked`, `enabled`, `count`, `truncated`, and the backend's own, such as a frame count |
| `set_property(window, item, name, value) -> bool` | False reports an error |
| `pointer(window, Point, PointerEvent) -> bool` | Press, release, move or wheel at a window position; false when the window is gone |
| `modifier(window, Modifier, pressed)` | Holds a modifier around a click |
| `input_text(window, text)`, `enter_key(window, key_code, &[Modifier])` | Keyboard input; key codes are Qt's `Qt::Key` |
| `async sleep(Duration)` | A pause on the GUI loop's own timers |
| `window_image(window) -> Result<WindowImage, String>` | Pixels of the whole window, or why there are none |
| `async click(window, item, Button, Clicks)` | Has a default, pressing and releasing at the centre of the frame; override it where the toolkit knows better where an item stands |

The futures are polled on the GUI thread and need not be `Send`. Positions and rectangles are
logical pixels; `WindowImage` is physical pixels and carries the scale factor between them.

### Plugging a backend in

1. Implement [`Backend`] on a type that is `Send + Sync`; keep whatever is thread-bound (windows,
   handles) in thread-locals, or reach it through the toolkit's own cross-thread handle.
2. Make `spawn_on_gui` post the closure to the toolkit's event loop and spawn the future it makes
   on the loop's executor.
3. Offer a `start` of your own that registers the windows and calls `spix_server::start`, as
   `spix_slint::start` does.

The tests of this crate run the server over a small fake backend (`src/fake.rs`) that logs what it
was asked, which is also the shortest example of an implementation.
