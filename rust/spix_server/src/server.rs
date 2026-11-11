// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The server: its listener thread, the little bit of HTTP it needs, and what each Spix method
//! does. The protocol is C++ Spix's (`docs/rpc-api.md`), down to the error strings that
//! `getErrors` returns: clients report them, and may retry the calls that make them.
//!
//! Everything but `wait` and `quit` runs on the GUI thread, handed there by the backend, because
//! the windows and everything the toolkit knows about them live there. `wait` sleeps on the
//! listener thread so that the app keeps running while it waits, and `quit` only tells the event
//! loop to stop, from wherever it is called.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::xmlrpc::{self, Call, Value};
use crate::{Backend, Button, Clicks, GuiTask, Modifier, Point, PointerEvent, Rect, crop};

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

/// Starts a Spix server on `127.0.0.1:port` over `backend`. Call it on the GUI thread, with the
/// backend's windows made and before the event loop runs; the calls it gets are handed to that
/// thread by [`Backend::spawn_on_gui`].
///
/// # Errors
///
/// If nothing can listen on `port`, or the thread that answers the calls can't be started.
pub fn start<B: Backend>(backend: B, port: u16) -> Result<(), Error> {
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|source| Error::Listen { port, source })?;
    let server = Arc::new(Server::new(backend));
    thread::Builder::new()
        .name("spix".to_owned())
        .spawn(move || serve(&listener, &server))
        .map_err(Error::Thread)?;
    Ok(())
}

/// The backend and the errors reported so far, shared by the listener thread and the tasks it
/// sends to the GUI thread.
struct Server<B: Backend> {
    backend: B,
    errors: Mutex<Vec<String>>,
}

/// The item a path found, with the window it is in.
struct Target<I> {
    window: String,
    item: I,
    /// The path names the window itself: there is no element behind it.
    is_window: bool,
}

impl<B: Backend> Server<B> {
    fn new(backend: B) -> Self {
        Self {
            backend,
            errors: Mutex::new(Vec::new()),
        }
    }

    /// Keeps `message` for the next `getErrors`, as C++ Spix keeps its command errors.
    fn report(&self, message: String) {
        if let Ok(mut errors) = self.errors.lock() {
            errors.push(message);
        }
    }

    fn errors(&self) -> Vec<String> {
        self.errors
            .lock()
            .map(|errors| errors.clone())
            .unwrap_or_default()
    }

    /// Decodes one request and runs it. Everything the protocol doesn't speak is answered with a
    /// fault, so a method nobody implemented never looks like a success.
    fn process(self: &Arc<Self>, body: &str) -> Result<Value, Fault> {
        let call = xmlrpc::decode_call(body).map_err(|error| match error {
            xmlrpc::Error::Unsupported => Fault::unsupported(),
            other => Fault::invalid(other.to_string()),
        })?;
        self.dispatch(call)
    }

    fn dispatch(self: &Arc<Self>, call: Call) -> Result<Value, Fault> {
        match call.method.as_str() {
            // Waiting lets the app run while the client waits, so it sleeps here and not on the
            // GUI thread, where it would stop everything it is meant to give time to.
            "wait" => {
                let milliseconds = u64::try_from(int_param(&call.params, 0)?).unwrap_or(0);
                thread::sleep(Duration::from_millis(milliseconds));
                Ok(empty())
            }
            // Told to the event loop from here: the answer goes out before the app can act on it,
            // which a client that waits for the app to end relies on.
            "quit" => {
                self.backend.quit();
                Ok(empty())
            }
            _ => self.on_gui(call),
        }
    }

    /// Runs `call` on the GUI thread and waits for its answer.
    ///
    /// It runs as a future, not as a plain call: a click awaits the toolkit's timers, and a timer
    /// only fires while the event loop runs.
    fn on_gui(self: &Arc<Self>, call: Call) -> Result<Value, Fault> {
        let (sender, receiver) = mpsc::channel();
        let server = Arc::clone(self);
        let task: GuiTask = Box::new(move || {
            Box::pin(async move {
                // The answer may be dropped if the client gave up first; that is its business.
                let _ = sender.send(server.execute(call).await);
            })
        });
        self.backend
            .spawn_on_gui(task)
            .map_err(|_| Fault::unreachable())?;
        // A task that was dropped without running drops its sender, which ends up here too.
        receiver.recv().map_err(|_| Fault::unreachable())?
    }

    /// Runs one call on the GUI thread, where the windows are. A path that isn't there is no
    /// fault: it goes into `getErrors`, as C++ Spix reports it, and the call answers with C++
    /// Spix's default.
    async fn execute(&self, call: Call) -> Result<Value, Fault> {
        let params = &call.params;
        match call.method.as_str() {
            "mouseClick" => {
                let path = string_param(params, 0)?;
                self.click(path, "ClickOnItem", Button::Left, 0, Clicks::Single)
                    .await;
                Ok(empty())
            }
            "mouseClickWithButton" => {
                let path = string_param(params, 0)?;
                let button =
                    Button::from_code(int_param(params, 1)?).ok_or_else(Fault::unsupported)?;
                let modifiers = int_param(params, 2)?;
                self.click(path, "ClickOnItem", button, modifiers, Clicks::Single)
                    .await;
                Ok(empty())
            }
            "mouseDoubleClick" => {
                let path = string_param(params, 0)?;
                self.click(path, "DoubleClickOnItem", Button::Left, 0, Clicks::Double)
                    .await;
                Ok(empty())
            }
            "mouseWheel" => self.mouse_wheel(params).await,
            "mouseDrag" => self.mouse_drag(params).await,
            "inputText" => {
                let path = string_param(params, 0)?;
                let text = string_param(params, 1)?;
                if let Some(target) = self.resolve(path, "InputText") {
                    self.backend.input_text(&target.window, text);
                }
                Ok(empty())
            }
            "enterKey" => {
                let path = string_param(params, 0)?;
                let key_code = int_param(params, 1)?;
                let modifiers = Modifier::from_flags(int_param(params, 2)?);
                if let Some(target) = self.resolve(path, "EnterKey") {
                    self.backend.enter_key(&target.window, key_code, &modifiers);
                }
                Ok(empty())
            }
            "existsAndVisible" => {
                let path = string_param(params, 0)?;
                // False for a path that isn't there, without an error: that is the answer, not a
                // failure.
                let visible = split_path(path).is_some_and(|(window, names)| {
                    self.backend
                        .find(window, &names)
                        .is_some_and(|item| self.backend.visible(window, &item))
                });
                Ok(Value::Bool(visible))
            }
            "getStringProperty" => {
                let path = string_param(params, 0)?;
                let name = string_param(params, 1)?;
                let value = self
                    .resolve(path, "GetProperty")
                    .and_then(|target| self.backend.property(&target.window, &target.item, name));
                Ok(Value::String(value.unwrap_or_default()))
            }
            "setStringProperty" => {
                let path = string_param(params, 0)?;
                let name = string_param(params, 1)?;
                let value = string_param(params, 2)?;
                if let Some(target) = self.resolve(path, "SetProperty")
                    && !self
                        .backend
                        .set_property(&target.window, &target.item, name, value)
                {
                    self.report(format!(
                        "setStringProperty: property '{name}' is not settable on a {} element",
                        B::TOOLKIT
                    ));
                }
                Ok(empty())
            }
            "getBoundingBox" => self.bounding_box(params),
            "takeScreenshot" => self.take_screenshot(params),
            "getErrors" => Ok(Value::Array(
                self.errors().into_iter().map(Value::String).collect(),
            )),
            // `wait` and `quit` are answered by `dispatch` without coming here; anything else is
            // outside the protocol.
            _ => Err(Fault::unsupported()),
        }
    }

    /// `getBoundingBox`: the item's bounds on the screen, or zeros for an item that isn't there.
    fn bounding_box(&self, params: &[Value]) -> Result<Value, Fault> {
        let path = string_param(params, 0)?;
        let bounds = self
            .resolve(path, "GetBoundingBox")
            .and_then(|target| self.backend.bounds(&target.window, &target.item))
            .unwrap_or(Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            });
        Ok(Value::Array(vec![
            Value::Double(bounds.x),
            Value::Double(bounds.y),
            Value::Double(bounds.width),
            Value::Double(bounds.height),
        ]))
    }

    /// Writes a PNG of the item at `path` to `file`, before the call answers so that the caller
    /// can read it at once. A path that isn't there, a backend without pixels or a file that
    /// can't be written goes into `getErrors`.
    fn take_screenshot(&self, params: &[Value]) -> Result<Value, Fault> {
        let (path, file) = (string_param(params, 0)?, string_param(params, 1)?);
        if let Some(target) = self.resolve(path, "TakeScreenshot") {
            let written = self
                .png_of(&target)
                .map_err(|error| error.to_string())
                .and_then(|png| std::fs::write(file, png).map_err(|error| error.to_string()));
            if let Err(error) = written {
                self.report(format!("TakeScreenshot: {path}: {error}"));
            }
        }
        Ok(empty())
    }

    fn png_of(&self, target: &Target<B::Item>) -> Result<Vec<u8>, crop::CropError> {
        let image = self
            .backend
            .window_image(&target.window)
            .map_err(crop::CropError::Image)?;
        let frame = self
            .backend
            .frame(&target.window, &target.item)
            .ok_or(crop::CropError::Empty)?;
        crop::png(&image, frame)
    }

    /// Clicks the item at `path`, holding the modifier keys down around it. The click itself is
    /// the backend's, which may know better where an item stands than the centre of its frame.
    ///
    /// The item is resolved first, so nothing is borrowed across the await.
    async fn click(
        &self,
        path: &str,
        command: &str,
        button: Button,
        modifiers: i32,
        clicks: Clicks,
    ) {
        let Some(target) = self.resolve(path, command) else {
            return;
        };
        if target.is_window {
            self.report(format!(
                "{command}: not supported on the window itself: {path}"
            ));
            return;
        }
        let keys = Modifier::from_flags(modifiers);
        for key in &keys {
            self.backend.modifier(&target.window, *key, true);
        }
        self.backend
            .click(&target.window, &target.item, button, clicks)
            .await;
        for key in keys.iter().rev() {
            self.backend.modifier(&target.window, *key, false);
        }
    }

    /// `mouseWheel`: validates the numbers, then turns the wheel (see [`Self::wheel`]).
    async fn mouse_wheel(&self, params: &[Value]) -> Result<Value, Fault> {
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
        self.wheel(path, angle_x, angle_y, steps, interval).await;
        Ok(empty())
    }

    /// `mouseDrag`: validates the numbers, then plays the stroke (see [`Self::drag`]).
    async fn mouse_drag(&self, params: &[Value]) -> Result<Value, Fault> {
        let path = string_param(params, 0)?;
        let from = (number_param(params, 1)?, number_param(params, 2)?);
        let to = (number_param(params, 3)?, number_param(params, 4)?);
        let steps = int_param(params, 5)?;
        let interval = int_param(params, 6)?;
        let hold = int_param(params, 7)?;
        // Bad numbers are refused as in `mouseWheel`.
        if steps < 1 || interval < 0 || hold < 0 {
            return Err(Fault::unsupported());
        }
        let milliseconds = |value: i32| Duration::from_millis(u64::from(value.unsigned_abs()));
        self.drag(
            path,
            from,
            to,
            steps,
            milliseconds(interval),
            milliseconds(hold),
        )
        .await;
        Ok(empty())
    }

    /// Drags with the left button over the item at `path`: presses at `from`, moves `steps` times
    /// in equal parts to `to`, holds still for `hold` and releases. The points are logical pixels
    /// from the item's top-left. The pauses are the backend's timers that the future awaits, so
    /// the event loop runs through them. Move `k` is due `k * interval` after the press and the
    /// release `steps * interval + hold` after it, each counted from the press: a timer that
    /// fires late delays only its own event.
    async fn drag(
        &self,
        path: &str,
        from: (f32, f32),
        to: (f32, f32),
        steps: i32,
        interval: Duration,
        hold: Duration,
    ) {
        let Some(target) = self.resolve(path, "Drag") else {
            return;
        };
        let send = |event: PointerEvent, at: (f32, f32)| {
            let Some(frame) = self.backend.frame(&target.window, &target.item) else {
                return false;
            };
            let at = Point {
                x: frame.x + f64::from(at.0),
                y: frame.y + f64::from(at.1),
            };
            self.backend.pointer(&target.window, at, event)
        };
        let start = Instant::now();
        if !send(PointerEvent::Press(Button::Left), from) {
            return;
        }
        for step in 1..=steps {
            self.sleep_until(start + interval * step.unsigned_abs())
                .await;
            // From the whole distance, not summed, so that rounding doesn't add up.
            #[allow(clippy::cast_precision_loss)] // Step counts are small.
            let done = step as f32 / steps as f32;
            let at = (
                from.0 + (to.0 - from.0) * done,
                from.1 + (to.1 - from.1) * done,
            );
            if !send(PointerEvent::Move, at) {
                return;
            }
        }
        self.sleep_until(start + interval * steps.unsigned_abs() + hold)
            .await;
        send(PointerEvent::Release(Button::Left), to);
    }

    /// Turns the wheel `steps` times over the centre of the item at `path`, `interval` apart. The
    /// pause is the backend's timer that the future awaits, so the event loop runs through it.
    async fn wheel(&self, path: &str, angle_x: i32, angle_y: i32, steps: i32, interval: Duration) {
        let Some(target) = self.resolve(path, "Wheel") else {
            return;
        };
        for step in 0..steps {
            if step > 0 {
                self.backend.sleep(interval).await;
            }
            let Some(frame) = self.backend.frame(&target.window, &target.item) else {
                return;
            };
            let at = Point {
                x: frame.x + frame.width / 2.0,
                y: frame.y + frame.height / 2.0,
            };
            if !self
                .backend
                .pointer(&target.window, at, PointerEvent::Wheel { angle_x, angle_y })
            {
                return;
            }
        }
    }

    /// Waits until `deadline` on the backend's timers, or not at all if it has passed. A timer may
    /// count whole milliseconds of its own clock and fire a little early, so it waits again for
    /// what is left: an event is never sent before its time.
    async fn sleep_until(&self, deadline: Instant) {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            self.backend.sleep(left).await;
        }
    }

    /// The item at `path` and the window that serves it. None when nothing matches, after
    /// recording `<command>: Item not found: <path>`: the error C++ Spix reports, which clients
    /// may retry on.
    fn resolve(&self, path: &str, command: &str) -> Option<Target<B::Item>> {
        let found = split_path(path).and_then(|(window, names)| {
            let item = self.backend.find(window, &names)?;
            Some(Target {
                window: window.to_owned(),
                item,
                is_window: names.is_empty(),
            })
        });
        if found.is_none() {
            self.report(format!("{command}: Item not found: {path}"));
        }
        found
    }
}

/// The listener thread: one connection at a time, since the tests talk to it one call at a time.
fn serve<B: Backend>(listener: &TcpListener, server: &Arc<Server<B>>) {
    for connection in listener.incoming() {
        let Ok(mut stream) = connection else {
            // A connection that couldn't be accepted: the client sees it close and gives up.
            continue;
        };
        // An answer that can't be written is lost with the connection it was written to.
        let _ = answer(&mut stream, server);
    }
}

/// Reads one request off `stream` and writes the answer back.
fn answer<B: Backend>(stream: &mut TcpStream, server: &Arc<Server<B>>) -> io::Result<()> {
    let mut reader = BufReader::new(&mut *stream);
    let body = read_request(&mut reader)?;
    let response = match server.process(&body) {
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

/// A path split into the name of the window and the ids that follow it. None for a path with no
/// window in it, which can't be found.
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

/// A coordinate, which a client may send as an int or a double.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)] // Pixels are small.
fn number_param(params: &[Value], index: usize) -> Result<f32, Fault> {
    match params.get(index) {
        Some(Value::Int(number)) => Ok(*number as f32),
        Some(Value::Double(number)) => Ok(*number as f32),
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
mod tests;
