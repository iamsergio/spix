// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The server: its listener thread, the little bit of HTTP it needs, and what each Spix method
//! does. The protocol is C++ Spix's (`docs/rpc-api.md`), down to the error strings that
//! `getErrors` returns: clients report them, and may retry the calls that make them.
//!
//! Everything but `wait` and `quit` runs on the GUI thread, handed there with
//! [`slint::invoke_from_event_loop`], because the windows and everything Slint knows about them
//! live there. `wait` sleeps on the listener thread so that the app keeps running while it waits,
//! and `quit` only tells the event loop to stop, from wherever it is called.

use std::cell::{Cell, RefCell};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::rc::Rc;
use std::sync::mpsc;
use std::task::{Poll, Waker};
use std::thread;
use std::time::Duration;

use i_slint_backend_testing::{ElementHandle, ElementRoot};
use slint::platform::PointerEventButton;
use slint::{ComponentHandle, Timer, Weak, Window};

use crate::input;
use crate::scene::{self, Item};
use crate::xmlrpc::{self, Call, Value};

/// Fault code for everything outside the protocol: a method Spix doesn't have, a parameter it
/// doesn't read.
const FAULT_UNSUPPORTED: i32 = -32601;
/// Fault code for a request that isn't a call of the protocol at all.
const FAULT_INVALID_REQUEST: i32 = -32600;

/// Why the server can't start.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("can't listen for Spix calls on 127.0.0.1:{port}")]
    Listen {
        port: u16,
        #[source]
        source: io::Error,
    },
    #[error("can't start the thread that answers Spix calls")]
    Thread(#[source] io::Error),
}

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
        register(name, window);
    }
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|source| Error::Listen { port, source })?;
    thread::Builder::new()
        .name("spix".to_owned())
        .spawn(move || serve(&listener))
        .map_err(Error::Thread)?;
    Ok(())
}

thread_local! {
    /// The windows the server answers for and the errors it has reported so far. Set by [`start`]
    /// on the GUI thread and only ever touched there: every call hops to it, `wait` and `quit`
    /// excepted because they touch neither.
    static SPIX: RefCell<Spix> = const { RefCell::new(Spix::new()) };
    /// The frame counters of the windows whose `frameCount` was read, by the address of the
    /// `Window`: a component's window lives as long as the component, and the counter with it.
    /// Apart from [`SPIX`], as they are read and added while a call holds that to find the window.
    static FRAMES: RefCell<Vec<(usize, Rc<Cell<u64>>)>> = const { RefCell::new(Vec::new()) };
}

struct Spix {
    windows: Vec<(String, Box<dyn WindowSource>)>,
    errors: Vec<String>,
}

impl Spix {
    const fn new() -> Self {
        Self {
            windows: Vec::new(),
            errors: Vec::new(),
        }
    }
}

/// One window of the app, with the type of its component erased.
trait WindowSource {
    /// Runs `f` with the window of the component and the root element to search from, while the
    /// component is still alive. False once it is gone.
    fn open(&self, f: &mut dyn FnMut(&Window, &ElementHandle) -> bool) -> bool;
}

/// The `Weak` of a component a caller handed to [`start`].
struct Registered<T: ComponentHandle>(Weak<T>);

impl<T: ComponentHandle> WindowSource for Registered<T> {
    fn open(&self, f: &mut dyn FnMut(&Window, &ElementHandle) -> bool) -> bool {
        let Some(component) = self.0.upgrade() else {
            return false;
        };
        f(component.window(), &component.root_element())
    }
}

fn register<T: ComponentHandle + 'static>(name: &str, window: Weak<T>) {
    SPIX.with(|spix| {
        spix.borrow_mut()
            .windows
            .push((name.to_owned(), Box::new(Registered(window))));
    });
}

/// What `getStringProperty` answers for `name` on `item`: what the scene knows, and the frame count
/// of a window, which lives here with the rendering notifiers.
fn read_property(window: &Window, item: &Item, name: &str) -> Option<String> {
    match (item, name) {
        (Item::Window, "frameCount") => frame_count(window),
        _ => scene::string_property(item, name),
    }
}

/// The frames `window` has drawn since the first time this was asked, which is 0. Counting starts
/// there, with a rendering notifier on the window, so an app whose tests never ask pays nothing.
///
/// `None` when the window can't be counted: its renderer has no notifier support (the testing
/// backend's software renderer), or the app already installed a notifier, a window having one.
fn frame_count(window: &Window) -> Option<String> {
    let key = std::ptr::from_ref(window) as usize;
    let known = FRAMES.with(|frames| {
        frames
            .borrow()
            .iter()
            .find(|(counted, _)| *counted == key)
            .map(|(_, frames)| frames.get())
    });
    if let Some(frames) = known {
        return Some(frames.to_string());
    }
    let frames = Rc::new(Cell::new(0_u64));
    let counter = Rc::clone(&frames);
    window
        .set_rendering_notifier(move |state, _api| {
            if matches!(state, slint::RenderingState::AfterRendering) {
                counter.set(counter.get() + 1);
            }
        })
        .ok()?;
    FRAMES.with(|counters| counters.borrow_mut().push((key, frames)));
    Some("0".to_owned())
}

/// Keeps `message` for the next [`getErrors`](execute), as C++ Spix keeps its command errors.
fn report(message: String) {
    SPIX.with(|spix| spix.borrow_mut().errors.push(message));
}

fn errors() -> Vec<String> {
    SPIX.with(|spix| spix.borrow().errors.clone())
}

/// The listener thread: one connection at a time, since the tests talk to it one call at a time.
fn serve(listener: &TcpListener) {
    for connection in listener.incoming() {
        let Ok(mut stream) = connection else {
            // A connection that couldn't be accepted: the client sees it close and gives up.
            continue;
        };
        // An answer that can't be written is lost with the connection it was written to.
        let _ = answer(&mut stream);
    }
}

/// Reads one request off `stream` and writes the answer back.
fn answer(stream: &mut TcpStream) -> io::Result<()> {
    let mut reader = BufReader::new(&mut *stream);
    let body = read_request(&mut reader)?;
    let response = match process(&body) {
        Ok(value) => xmlrpc::encode_response(&value),
        Err(fault) => xmlrpc::encode_fault(fault.code, &fault.message),
    };
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
        response.len()
    )?;
    stream.flush()
}

/// Reads the body of one HTTP request: a `POST` with a `Content-Length` header, which is all
/// an XML-RPC client over `ureq` sends.
fn read_request(reader: &mut impl BufRead) -> io::Result<String> {
    let mut line = String::new();
    reader.read_line(&mut line)?; // The request line: POST /RPC2 HTTP/1.1.
    let mut length = 0;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// Decodes one request and runs it. Everything the protocol doesn't speak is answered with a
/// fault, so a method nobody implemented never looks like a success.
fn process(body: &str) -> Result<Value, Fault> {
    let call = xmlrpc::decode_call(body).map_err(|error| match error {
        xmlrpc::Error::Unsupported => Fault::unsupported(),
        other => Fault::invalid(other.to_string()),
    })?;
    dispatch(&call)
}

fn dispatch(call: &Call) -> Result<Value, Fault> {
    match call.method.as_str() {
        // Waiting lets the app run while the client waits, so it sleeps here and not on the GUI
        // thread, where it would stop everything it is meant to give time to.
        "wait" => {
            let milliseconds = u64::try_from(int_param(&call.params, 0)?).unwrap_or(0);
            thread::sleep(Duration::from_millis(milliseconds));
            Ok(empty())
        }
        // Told to the event loop from here: the answer goes out before the app can act on it,
        // which is what `App::quit_and_wait` reads.
        "quit" => {
            let _ = slint::quit_event_loop();
            Ok(empty())
        }
        _ => on_gui(call.clone()),
    }
}

/// Runs `call` on the GUI thread and waits for its answer.
///
/// It runs as a future, not inside this closure: a click awaits Slint's own `single_click`, which
/// sleeps on a timer, and a timer only fires while the event loop runs.
fn on_gui(call: Call) -> Result<Value, Fault> {
    let (sender, receiver) = mpsc::channel();
    slint::invoke_from_event_loop(move || {
        let for_the_future = sender.clone();
        let spawned = slint::spawn_local(async move {
            // The answer may be dropped if the client gave up first; that is its business.
            let _ = for_the_future.send(execute(call).await);
        });
        if spawned.is_err() {
            // Can't happen: the event loop proxy `invoke_from_event_loop` just used is still
            // there. The listener gets a fault instead of waiting for an answer forever.
            let _ = sender.send(Err(Fault::unreachable()));
        }
    })
    .map_err(|_| Fault::unreachable())?;
    receiver.recv().map_err(|_| Fault::unreachable())?
}

/// Runs one call on the GUI thread, where the windows are. A path that isn't there is no fault: it
/// goes into `getErrors`, as C++ Spix reports it, and the call answers with C++ Spix's default.
async fn execute(call: Call) -> Result<Value, Fault> {
    let params = &call.params;
    match call.method.as_str() {
        "mouseClick" => {
            let path = string_param(params, 0)?;
            click(path, "ClickOnItem", PointerEventButton::Left, 0).await;
            Ok(empty())
        }
        "mouseClickWithButton" => {
            let path = string_param(params, 0)?;
            let button = input::button(int_param(params, 1)?).ok_or_else(Fault::unsupported)?;
            let modifiers = int_param(params, 2)?;
            click(path, "ClickOnItem", button, modifiers).await;
            Ok(empty())
        }
        "mouseDoubleClick" => {
            let path = string_param(params, 0)?;
            double_click(path, "DoubleClickOnItem").await;
            Ok(empty())
        }
        "mouseWheel" => mouse_wheel(params).await,
        "inputText" => {
            let path = string_param(params, 0)?;
            let text = string_param(params, 1)?;
            with_item(path, "InputText", &mut |window, _| {
                input::input_text(window, text);
            });
            Ok(empty())
        }
        "enterKey" => {
            let path = string_param(params, 0)?;
            let key_code = int_param(params, 1)?;
            let modifiers = int_param(params, 2)?;
            with_item(path, "EnterKey", &mut |window, _| {
                // A key Slint has no key for is dropped, as C++ Spix drops it.
                let _ = input::enter_key(window, key_code, modifiers);
            });
            Ok(empty())
        }
        "existsAndVisible" => {
            let path = string_param(params, 0)?;
            // False for a path that isn't there, without an error: that is the answer, not a failure.
            let mut visible = false;
            if let Some((window_name, names)) = split_path(path) {
                with_window(window_name, &mut |window, root| {
                    if let Some(item) = scene::find_item(root, &names) {
                        visible = scene::visible(window, &item);
                    }
                    true
                });
            }
            Ok(Value::Bool(visible))
        }
        "getStringProperty" => {
            let path = string_param(params, 0)?;
            let name = string_param(params, 1)?;
            let mut value = None;
            with_item(path, "GetProperty", &mut |window, item| {
                value = read_property(window, item, name);
            });
            Ok(Value::String(value.unwrap_or_default()))
        }
        "setStringProperty" => {
            let path = string_param(params, 0)?;
            let name = string_param(params, 1)?;
            let value = string_param(params, 2)?;
            let mut set = false;
            let found = with_item(path, "SetProperty", &mut |_window, item| {
                set = scene::set_string_property(item, name, value);
            });
            if found && !set {
                report(format!(
                    "setStringProperty: property '{name}' is not settable on a Slint element"
                ));
            }
            Ok(empty())
        }
        "getBoundingBox" => {
            let path = string_param(params, 0)?;
            let mut bounds = scene::Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            };
            with_item(path, "GetBoundingBox", &mut |window, item| {
                bounds = scene::bounds(window, item);
            });
            Ok(Value::Array(vec![
                Value::Double(bounds.x),
                Value::Double(bounds.y),
                Value::Double(bounds.width),
                Value::Double(bounds.height),
            ]))
        }
        "takeScreenshot" => take_screenshot(params),
        "getErrors" => Ok(Value::Array(
            errors().into_iter().map(Value::String).collect(),
        )),
        // `wait` and `quit` are answered by [`dispatch`] without coming here; anything else is
        // outside the protocol.
        _ => Err(Fault::unsupported()),
    }
}

/// Writes a PNG of the item at `path` to `file`, before the call answers so that the caller can
/// read it at once. A path that isn't there, a renderer without pixels or a file that can't be
/// written goes into `getErrors`.
fn take_screenshot(params: &[Value]) -> Result<Value, Fault> {
    let (path, file) = (string_param(params, 0)?, string_param(params, 1)?);
    with_item(path, "TakeScreenshot", &mut |window, item| {
        let written = scene::screenshot_png(window, item)
            .map_err(|error| error.to_string())
            .and_then(|png| std::fs::write(file, png).map_err(|error| error.to_string()));
        if let Err(error) = written {
            report(format!("TakeScreenshot: {path}: {error}"));
        }
    });
    Ok(empty())
}

/// Clicks the item at `path` with Slint's own `single_click`, which knows where an element inside
/// an open popup stands, holding the modifier keys down around it.
///
/// The item is resolved first and the registry is let go, so nothing is borrowed across the await.
async fn click(path: &str, command: &str, button: PointerEventButton, modifiers: i32) {
    let Some((window_name, item)) = resolve(path, command) else {
        return;
    };
    let Item::Element(element) = item else {
        report(format!(
            "{command}: not supported on the window itself: {path}"
        ));
        return;
    };
    let keys = input::modifier_keys(modifiers);
    with_window(&window_name, &mut |window, _| {
        for key in &keys {
            input::press(window, *key);
        }
        true
    });
    element.single_click(button).await;
    with_window(&window_name, &mut |window, _| {
        for key in keys.iter().rev() {
            input::release(window, *key);
        }
        true
    });
}

/// `mouseWheel`: validates the numbers, then turns the wheel (see [`wheel`]).
async fn mouse_wheel(params: &[Value]) -> Result<Value, Fault> {
    let path = string_param(params, 0)?;
    let angle_x = int_param(params, 1)?;
    let angle_y = int_param(params, 2)?;
    let steps = int_param(params, 3)?;
    let interval = int_param(params, 4)?;
    // Bad numbers are refused as a button that doesn't exist is.
    if steps < 1 || interval < 0 {
        return Err(Fault::unsupported());
    }
    let interval = Duration::from_millis(u64::from(interval.unsigned_abs()));
    wheel(path, angle_x, angle_y, steps, interval).await;
    Ok(empty())
}

/// Turns the wheel `steps` times over the centre of the item at `path`, `interval` apart. The
/// pause is a Slint timer that the future awaits, so the event loop runs through it.
async fn wheel(path: &str, angle_x: i32, angle_y: i32, steps: i32, interval: Duration) {
    let Some((window_name, item)) = resolve(path, "Wheel") else {
        return;
    };
    for step in 0..steps {
        if step > 0 {
            sleep(interval).await;
        }
        let alive = with_window(&window_name, &mut |window, _| {
            input::wheel(window, scene::center(window, &item), angle_x, angle_y);
            true
        });
        if !alive {
            return;
        }
    }
}

/// Waits `duration` on a Slint timer, which only fires while the event loop runs.
async fn sleep(duration: Duration) {
    let state = Rc::new(RefCell::new((false, None::<Waker>)));
    let timer_state = Rc::clone(&state);
    Timer::single_shot(duration, move || {
        let mut state = timer_state.borrow_mut();
        state.0 = true;
        if let Some(waker) = state.1.take() {
            waker.wake();
        }
    });
    std::future::poll_fn(move |context| {
        let mut state = state.borrow_mut();
        if state.0 {
            Poll::Ready(())
        } else {
            state.1 = Some(context.waker().clone());
            Poll::Pending
        }
    })
    .await;
}

/// Double-clicks the item at `path` with Slint's own `double_click` (see [`click`]).
async fn double_click(path: &str, command: &str) {
    let Some((_, item)) = resolve(path, command) else {
        return;
    };
    let Item::Element(element) = item else {
        report(format!(
            "{command}: not supported on the window itself: {path}"
        ));
        return;
    };
    element.double_click(PointerEventButton::Left).await;
}

/// The item at `path` and the window that serves it, both owned: the registry is only borrowed
/// while this runs, which is what lets [`click`] await afterwards.
fn resolve(path: &str, command: &str) -> Option<(String, Item)> {
    let mut found = None;
    if let Some((window_name, names)) = split_path(path) {
        with_window(window_name, &mut |_window, root| {
            if let Some(item) = scene::find_item(root, &names) {
                found = Some((window_name.to_owned(), item));
            }
            found.is_some()
        });
    }
    if found.is_none() {
        report(format!("{command}: Item not found: {path}"));
    }
    found
}

/// Runs `f` with the item at `path`, in the window its first segment names. False when nothing
/// matches, after recording `<command>: Item not found: <path>`: the error C++ Spix reports, which
/// clients may retry on.
fn with_item(path: &str, command: &str, f: &mut dyn FnMut(&Window, &Item)) -> bool {
    let mut found = false;
    if let Some((window_name, names)) = split_path(path) {
        with_window(window_name, &mut |window, root| {
            if let Some(item) = scene::find_item(root, &names) {
                f(window, &item);
                found = true;
            }
            found
        });
    }
    if !found {
        report(format!("{command}: Item not found: {path}"));
    }
    found
}

/// Runs `f` with the window registered under `name` and the root element to search from, while the
/// component behind it is still alive. False when there is no such window.
fn with_window(name: &str, f: &mut dyn FnMut(&Window, &ElementHandle) -> bool) -> bool {
    SPIX.with(|spix| {
        let spix = spix.borrow();
        let Some((_, window)) = spix
            .windows
            .iter()
            .find(|(registered, _)| registered == name)
        else {
            return false;
        };
        window.open(f)
    })
}

/// A path split into the name of the window and the `accessible-id`s that follow it. None for a
/// path with no window in it, which can't be found.
fn split_path(path: &str) -> Option<(&str, Vec<&str>)> {
    let mut segments = path.split('/');
    let window = segments.next()?;
    if window.is_empty() {
        return None;
    }
    Some((window, segments.collect()))
}

/// One parameter as the string the calls carry it in.
fn string_param(params: &[Value], index: usize) -> Result<&str, Fault> {
    match params.get(index) {
        Some(Value::String(text)) => Ok(text),
        _ => Err(Fault::unsupported()),
    }
}

/// One parameter as the number the calls carry it in.
fn int_param(params: &[Value], index: usize) -> Result<i32, Fault> {
    match params.get(index) {
        Some(Value::Int(number)) => Ok(*number),
        _ => Err(Fault::unsupported()),
    }
}

/// What a call that returns nothing answers with, as Spix answers it.
fn empty() -> Value {
    Value::String(String::new())
}

/// An answer that isn't a value: an XML-RPC fault, with the code and the string in it.
#[derive(Debug)]
struct Fault {
    code: i32,
    message: String,
}

impl Fault {
    fn unsupported() -> Self {
        Self {
            code: FAULT_UNSUPPORTED,
            message: "unsupported".to_owned(),
        }
    }

    fn invalid(message: String) -> Self {
        Self {
            code: FAULT_INVALID_REQUEST,
            message,
        }
    }

    /// The GUI thread can't be reached: the app is on its way out.
    fn unreachable() -> Self {
        Self::invalid("the event loop is gone".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(method: &str, params: Vec<Value>) -> Call {
        Call {
            method: method.to_owned(),
            params,
        }
    }

    fn path_param(path: &str) -> Vec<Value> {
        vec![Value::String(path.to_owned())]
    }

    /// Runs one call of `execute` to completion: what these tests cover has nothing to await (the
    /// element isn't there), so the future is ready on its first poll and the unit tests need no
    /// event loop. The clicks that do await are over the wire, in `tests/server.rs`.
    fn run(method: &str, params: Vec<Value>) -> Result<Value, Fault> {
        let mut future = core::pin::pin!(execute(call(method, params)));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(answer) => answer,
            std::task::Poll::Pending => {
                panic!("nothing here may await: the unit tests have no event loop")
            }
        }
    }

    // INFRA-004
    #[test]
    fn reports_a_path_that_is_not_there_the_way_spix_does() {
        let read = run(
            "getStringProperty",
            vec![
                Value::String("mainWindow/nope".to_owned()),
                Value::String("text".to_owned()),
            ],
        )
        .unwrap();
        assert_eq!(read, Value::String(String::new()));
        assert_eq!(
            errors(),
            vec!["GetProperty: Item not found: mainWindow/nope".to_owned()]
        );

        let bounds = run("getBoundingBox", path_param("elsewhere/item")).unwrap();
        assert_eq!(
            bounds,
            Value::Array(vec![
                Value::Double(0.0),
                Value::Double(0.0),
                Value::Double(0.0),
                Value::Double(0.0)
            ])
        );
        assert_eq!(
            errors(),
            vec![
                "GetProperty: Item not found: mainWindow/nope".to_owned(),
                "GetBoundingBox: Item not found: elsewhere/item".to_owned()
            ]
        );
    }

    // INFRA-004
    #[test]
    fn a_missing_path_is_not_an_error_for_exists_and_visible() {
        let answer = run("existsAndVisible", path_param("mainWindow/nothing")).unwrap();
        assert_eq!(answer, Value::Bool(false));
        assert!(errors().is_empty(), "{:?}", errors());
    }

    // INFRA-004
    #[test]
    fn clicks_and_keys_on_a_path_that_is_not_there_leave_the_spix_error() {
        run("mouseClick", path_param("mainWindow/nowhere")).unwrap();
        run(
            "mouseClickWithButton",
            vec![
                Value::String("mainWindow/nowhere".to_owned()),
                Value::Int(1),
                Value::Int(0),
            ],
        )
        .unwrap();
        run(
            "enterKey",
            vec![
                Value::String("mainWindow/nowhere".to_owned()),
                Value::Int(0x0100_0004),
                Value::Int(0),
            ],
        )
        .unwrap();
        run(
            "inputText",
            vec![
                Value::String("mainWindow/nowhere".to_owned()),
                Value::String("hello".to_owned()),
            ],
        )
        .unwrap();
        assert_eq!(
            errors(),
            vec![
                "ClickOnItem: Item not found: mainWindow/nowhere".to_owned(),
                "ClickOnItem: Item not found: mainWindow/nowhere".to_owned(),
                "EnterKey: Item not found: mainWindow/nowhere".to_owned(),
                "InputText: Item not found: mainWindow/nowhere".to_owned(),
            ]
        );
    }

    // INFRA-004
    #[test]
    fn a_screenshot_of_a_path_that_is_not_there_leaves_the_spix_error() {
        run(
            "takeScreenshot",
            vec![
                Value::String("mainWindow/nowhere".to_owned()),
                Value::String("unused.png".to_owned()),
            ],
        )
        .unwrap();
        assert_eq!(
            errors(),
            vec!["TakeScreenshot: Item not found: mainWindow/nowhere".to_owned()]
        );
    }

    // INFRA-004
    #[test]
    fn get_errors_keeps_what_it_has_reported_so_far() {
        run("mouseClick", path_param("mainWindow/nowhere")).unwrap();
        let reported = run("getErrors", vec![]).unwrap();
        assert_eq!(
            reported,
            Value::Array(vec![Value::String(
                "ClickOnItem: Item not found: mainWindow/nowhere".to_owned()
            )])
        );
    }

    // INFRA-004
    #[test]
    fn answers_a_fault_for_everything_outside_the_protocol() {
        // Through `execute`, as the other calls here: `dispatch` hands everything but `wait` and
        // `quit` to the event loop, and this test has none.
        let unknown = run(
            "takeScreenshotAsBase64",
            vec![Value::String("a".to_owned())],
        )
        .unwrap_err();
        assert_eq!(unknown.code, FAULT_UNSUPPORTED);
        assert_eq!(unknown.message, "unsupported");

        let malformed = process("not xml at all").unwrap_err();
        assert_eq!(malformed.code, FAULT_INVALID_REQUEST);

        let array = process("<?xml version=\"1.0\"?><methodCall><methodName>m</methodName><params><param><value><array><data></data></array></value></param></params></methodCall>").unwrap_err();
        assert_eq!(array.code, FAULT_UNSUPPORTED);
        assert_eq!(array.message, "unsupported");
    }

    // INFRA-004
    #[test]
    fn reads_the_body_of_the_request_ureq_sends() {
        let request = "POST /RPC2 HTTP/1.1\r\nHost: 127.0.0.1:80\r\nContent-Type: text/xml\r\nContent-Length: 7\r\nConnection: keep-alive\r\n\r\n<ping/>";
        let body = read_request(&mut request.as_bytes()).unwrap();
        assert_eq!(body, "<ping/>");
    }
}
