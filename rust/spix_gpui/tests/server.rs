// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The server over a real socket: one call of every method the server speaks, the faults for what
//! the protocol doesn't speak, and `quit` ending the app's loop the way it ends a real app's.
//! GPUI runs one app per process, so this binary holds one test. Without a display GPUI uses its
//! headless platform, which draws nothing, so the screenshot part checks the error instead.

mod support;

use std::cell::RefCell;
use std::fmt::Write as _;
use std::net::{TcpListener, TcpStream};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

/// What one call answered: the text of its value, its boolean, or the entries of an array read as
/// text (bounding boxes come as numbers, `getErrors` as strings).
#[derive(Debug, PartialEq)]
enum Answer {
    Text(String),
    Bool(bool),
    Array(Vec<String>),
}

/// Asks the app to quit when the client thread ends, however it ends: the app's loop waits for
/// it, and a client that gave up would otherwise leave it waiting forever.
struct QuitWhenDone(u16);

impl Drop for QuitWhenDone {
    fn drop(&mut self) {
        let _ = rpc(self.0, "quit", &[]);
    }
}

fn has_display() -> bool {
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[test]
fn answers_the_calls_of_the_gui_tests_and_quits() -> Result<(), String> {
    let port = free_port()?;
    let client = thread::spawn(move || drive(port));
    let started: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let failure = Rc::clone(&started);

    gpui_kit::platform::application().run(move |cx| {
        let opened = support::open(cx);
        let result = opened.and_then(|(handle, _view)| {
            spix_gpui::start(port, [("mainWindow", handle)], cx).map_err(|error| error.to_string())
        });
        match result {
            Ok(()) => cx.activate(true),
            Err(error) => {
                *failure.borrow_mut() = Some(error);
                cx.quit();
            }
        }
    });

    if let Some(error) = started.borrow().clone() {
        return Err(error);
    }
    client
        .join()
        .map_err(|_| "the client thread panicked".to_owned())?
}

/// Makes every call of the protocol against `port`, in the order the tests make them.
fn drive(port: u16) -> Result<(), String> {
    // The app may still be opening its window and starting the listener.
    let deadline = Instant::now() + Duration::from_secs(20);
    while rpc(port, "wait", &[value("<int>0</int>")]).is_err() {
        if Instant::now() > deadline {
            return Err("the server never answered".to_owned());
        }
        thread::sleep(Duration::from_millis(50));
    }
    let _quit = QuitWhenDone(port);
    // Let the first frame be drawn: the registry is filled while one paints.
    call(port, "wait", &[value("<int>300</int>")])?;

    existence(port)?;
    properties(port)?;
    clicks(port)?;
    text(port)?;
    waiting(port)?;
    wheel(port)?;
    drag(port)?;
    menu(port)?;
    screenshots(port)?;
    errors(port)?;

    // The guard quits once more, harmlessly: the first answer comes before the loop ends.
    assert_eq!(call(port, "quit", &[])?, Answer::Text(String::new()));
    Ok(())
}

/// What the status label of the view says.
fn status(port: u16) -> Result<String, String> {
    text_of_item(port, "mainWindow/status")
}

fn exists(port: u16, at: &str) -> Result<bool, String> {
    match call(port, "existsAndVisible", &[path(at)])? {
        Answer::Bool(found) => Ok(found),
        other => Err(format!("existsAndVisible of {at} is {other:?}")),
    }
}

fn click_button(port: u16, at: &str, button: i32, modifiers: i32) -> Result<(), String> {
    call(
        port,
        "mouseClickWithButton",
        &[
            path(at),
            value(&format!("<int>{button}</int>")),
            value(&format!("<int>{modifiers}</int>")),
        ],
    )
    .map(|_| ())
}

/// What is there, what isn't, and the window itself; paths through ancestors.
fn existence(port: u16) -> Result<(), String> {
    assert!(exists(port, "mainWindow/label")?);
    assert!(exists(port, "mainWindow/root/label")?);
    assert!(exists(port, "mainWindow/toolbar/button")?);
    assert!(!exists(port, "mainWindow/nothing")?);
    assert!(!exists(port, "mainWindow/toolbar/label")?);
    assert!(!exists(port, "nowindow/label")?);
    assert!(exists(port, "mainWindow")?);
    // A row that is not painted, as the list is virtual.
    assert!(exists(port, "mainWindow/list/row-0")?);
    assert!(!exists(port, "mainWindow/list/row-150")?);
    Ok(())
}

/// What the tests read from an element, and where it stands.
fn properties(port: u16) -> Result<(), String> {
    assert_eq!(text_of_item(port, "mainWindow/label")?, "Hello");
    assert_eq!(string(port, "mainWindow/button", "enabled")?, "true");
    assert_eq!(string(port, "mainWindow/off", "enabled")?, "false");
    assert_eq!(string(port, "mainWindow/check", "checked")?, "false");
    // The list's `count` comes from the app's property hook, which knows the model.
    assert_eq!(
        string(port, "mainWindow/list", "count")?,
        support::ROWS.to_string()
    );
    assert_eq!(string(port, "mainWindow/list", "unknown")?, "");

    let bounds = call(port, "getBoundingBox", &[path("mainWindow/button")])?;
    let Answer::Array(bounds) = bounds else {
        return Err(format!("the bounding box of a button is {bounds:?}"));
    };
    assert_eq!(bounds.len(), 4, "{bounds:?}");
    assert!(
        bounds[2].parse::<f64>().unwrap_or(0.0) > 0.0
            && bounds[3].parse::<f64>().unwrap_or(0.0) > 0.0,
        "{bounds:?}"
    );
    Ok(())
}

/// Clicks reach the buttons: single, double, with a modifier, with the right button.
fn clicks(port: u16) -> Result<(), String> {
    call(port, "mouseClick", &[path("mainWindow/button")])?;
    assert!(status(port)?.contains("clicks=1 "), "{}", status(port)?);

    call(port, "mouseDoubleClick", &[path("mainWindow/button")])?;
    let seen = status(port)?;
    assert!(
        seen.contains("clicks=3 ") && seen.contains("double=1 "),
        "{seen}"
    );

    // Shift is bit 1.
    click_button(port, "mainWindow/button", 1, 1)?;
    let seen = status(port)?;
    assert!(
        seen.contains("clicks=4 ") && seen.contains("shift=1 "),
        "{seen}"
    );
    click_button(port, "mainWindow/button", 1, 0)?;
    assert!(status(port)?.contains("shift=1 "), "shift stayed down");

    call(port, "mouseClick", &[path("mainWindow/check")])?;
    assert_eq!(string(port, "mainWindow/check", "checked")?, "true");
    Ok(())
}

/// Text: a click focuses the field, the keys go to it, and a set property is read back.
fn text(port: u16) -> Result<(), String> {
    call(port, "mouseClick", &[path("mainWindow/field")])?;
    call(
        port,
        "inputText",
        &[path("mainWindow/field"), value("<string>hi ü</string>")],
    )?;
    assert_eq!(text_of_item(port, "mainWindow/field")?, "hi ü");

    // Backspace, then Return (the Qt codes), which the input answers with its enter event.
    for key in [0x0100_0003, 0x0100_0004] {
        call(
            port,
            "enterKey",
            &[
                path("mainWindow/field"),
                value(&format!("<int>{key}</int>")),
                value("<int>0</int>"),
            ],
        )?;
    }
    call(port, "wait", &[value("<int>50</int>")])?;
    assert_eq!(text_of_item(port, "mainWindow/field")?, "hi ");
    assert!(status(port)?.contains("enters=1 "), "{}", status(port)?);

    call(
        port,
        "setStringProperty",
        &[
            path("mainWindow/field"),
            property("text"),
            value("<string>set</string>"),
        ],
    )?;
    assert_eq!(text_of_item(port, "mainWindow/field")?, "set");

    // Two inputs with one id under different parents, each set through its own path.
    for (scope, word) in [("roomComposer", "room"), ("threadPane", "thread")] {
        call(
            port,
            "setStringProperty",
            &[
                path(&format!("mainWindow/{scope}/composerField")),
                property("text"),
                value(&format!("<string>{word}</string>")),
            ],
        )?;
    }
    assert_eq!(
        text_of_item(port, "mainWindow/roomComposer/composerField")?,
        "room"
    );
    assert_eq!(
        text_of_item(port, "mainWindow/threadPane/composerField")?,
        "thread"
    );

    // A property that nothing registered can't be set, which is an error to read, not a fault.
    call(
        port,
        "setStringProperty",
        &[
            path("mainWindow/list"),
            property("count"),
            value("<string>1</string>"),
        ],
    )?;
    assert_eq!(
        errors_list(port)?,
        vec!["setStringProperty: property 'count' is not settable on a GPUI element".to_owned()]
    );
    Ok(())
}

/// Waiting waits: the answer only comes once the time is up.
fn waiting(port: u16) -> Result<(), String> {
    let waited = Instant::now();
    call(port, "wait", &[value("<int>60</int>")])?;
    let waited = waited.elapsed();
    assert!(
        waited >= Duration::from_millis(50),
        "wait returned after {waited:?}"
    );
    Ok(())
}

/// What an element says its `text` is.
fn text_of_item(port: u16, at: &str) -> Result<String, String> {
    match call(port, "getStringProperty", &[path(at), property("text")])? {
        Answer::Text(text) => Ok(text),
        other => Err(format!("the text of {at} is {other:?}")),
    }
}

/// A string property of an element.
fn string(port: u16, at: &str, name: &str) -> Result<String, String> {
    match call(port, "getStringProperty", &[path(at), property(name)])? {
        Answer::Text(text) => Ok(text),
        other => Err(format!("the {name} of {at} is {other:?}")),
    }
}

/// What `getErrors` holds.
fn errors_list(port: u16) -> Result<Vec<String>, String> {
    match call(port, "getErrors", &[])? {
        Answer::Array(errors) => Ok(errors),
        other => Err(format!("getErrors is {other:?}")),
    }
}

/// One call of the wheel: `steps` turns of `angle_y` over the list, `interval` milliseconds apart.
fn wheel_call(port: u16, angle_y: i32, steps: i32, interval: i32) -> Result<Answer, String> {
    call(
        port,
        "mouseWheel",
        &[
            path("mainWindow/list"),
            value("<int>0</int>"),
            value(&format!("<int>{angle_y}</int>")),
            value(&format!("<int>{steps}</int>")),
            value(&format!("<int>{interval}</int>")),
        ],
    )
}

/// The wheel scrolls a virtual list: a negative y moves the content towards its end and a
/// positive one back, and the steps are spread over the interval between them.
fn wheel(port: u16) -> Result<(), String> {
    assert!(exists(port, "mainWindow/list/row-0")?);

    let started = Instant::now();
    wheel_call(port, -120, 5, 20)?;
    assert!(
        started.elapsed() >= Duration::from_millis(80),
        "the call returned after {:?}, before the intervals were up",
        started.elapsed()
    );
    assert!(
        !exists(port, "mainWindow/list/row-0")?,
        "a negative y scrolls towards the end"
    );
    assert!(exists(port, "mainWindow/list/row-13")?);

    wheel_call(port, 120, 5, 0)?;
    assert!(
        exists(port, "mainWindow/list/row-0")?,
        "a positive y scrolls back"
    );

    for bad in [(0, 0), (-1, 0), (1, -1)] {
        assert_eq!(
            wheel_call(port, -120, bad.0, bad.1),
            Err("unsupported".to_owned()),
            "steps {} and interval {}",
            bad.0,
            bad.1
        );
    }
    Ok(())
}

/// One call of the drag over the pad: from (40, `from_y`) to (40, `to_y`) in ten steps.
fn drag_call(port: u16, from_y: i32, to_y: i32) -> Result<Answer, String> {
    call(
        port,
        "mouseDrag",
        &[
            path("mainWindow/pad"),
            value("<int>40</int>"),
            value(&format!("<double>{from_y}.0</double>")),
            value("<double>40.0</double>"),
            value(&format!("<int>{to_y}</int>")),
            value("<int>10</int>"),
            value("<int>10</int>"),
            value("<int>50</int>"),
        ],
    )
}

/// Where the pointer got to on the pad, in window pixels, as the status says.
fn dragged(port: u16) -> Result<f64, String> {
    let status = status(port)?;
    let word = status
        .split_whitespace()
        .find_map(|word| word.strip_prefix("dragged="))
        .ok_or("no dragged in the status")?;
    word.parse().map_err(|_| format!("dragged is {word}"))
}

/// A drag presses, moves with the button held and releases: the pad sees the pointer move by as
/// much as the drag did.
fn drag(port: u16) -> Result<(), String> {
    drag_call(port, 20, 50)?;
    let near = dragged(port)?;
    drag_call(port, 20, 90)?;
    let far = dragged(port)?;
    assert!(
        ((far - near) - 40.0).abs() <= 1.0,
        "dragged to {near}, then to {far}"
    );
    assert_eq!(
        drag_call(port, 20, 50).and(call(
            port,
            "mouseDrag",
            &[
                path("mainWindow/pad"),
                value("<int>0</int>"),
                value("<int>0</int>"),
                value("<int>0</int>"),
                value("<int>0</int>"),
                value("<int>0</int>"),
                value("<int>0</int>"),
                value("<int>0</int>"),
            ],
        )),
        Err("unsupported".to_owned()),
        "no steps"
    );
    Ok(())
}

/// A context menu, which GPUI paints under a root of its own: the right click opens it, and the
/// click on its item reaches into the popup and runs the item's own handler.
fn menu(port: u16) -> Result<(), String> {
    assert!(
        !exists(port, "mainWindow/menu-item")?,
        "no menu before anything opened it"
    );

    click_button(port, "mainWindow/anchor", 2, 0)?;
    assert!(
        exists(port, "mainWindow/menu/menu-item")?,
        "a right click opens the menu"
    );
    assert_eq!(text_of_item(port, "mainWindow/menu-item")?, "Menu item");

    call(port, "mouseClick", &[path("mainWindow/menu-item")])?;
    assert!(status(port)?.contains("menu=1 "), "{}", status(port)?);
    assert!(
        !exists(port, "mainWindow/menu-item")?,
        "the item closed the menu"
    );
    Ok(())
}

/// A screenshot of an element is a PNG of its size, with something in it, written before the call
/// answers. Without a display there are no pixels, and the call says so in `getErrors`.
fn screenshots(port: u16) -> Result<(), String> {
    let file = std::env::temp_dir().join(format!("spix_gpui_button_{}.png", std::process::id()));
    let file_name = file.to_string_lossy().into_owned();
    let before = errors_list(port)?.len();
    assert_eq!(
        call(
            port,
            "takeScreenshot",
            &[path("mainWindow/button"), path(&file_name)]
        )?,
        Answer::Text(String::new())
    );
    if !has_display() {
        println!("screenshot checked for its error only: no display, GPUI draws nothing");
        let errors = errors_list(port)?;
        assert_eq!(errors.len(), before + 1, "{errors:?}");
        assert!(errors[before].starts_with("TakeScreenshot: "), "{errors:?}");
        return Ok(());
    }
    assert_eq!(errors_list(port)?.len(), before, "{:?}", errors_list(port)?);

    let Answer::Array(bounds) = call(port, "getBoundingBox", &[path("mainWindow/button")])? else {
        return Err("no bounding box".to_owned());
    };
    let logical: Vec<f64> = bounds.iter().map(|n| n.parse().unwrap_or(0.0)).collect();

    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&file).map_err(|error| error.to_string())?,
    ));
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("no buffer size")?];
    let info = reader
        .next_frame(&mut pixels)
        .map_err(|error| error.to_string())?;
    let _ = std::fs::remove_file(&file);
    // Physical pixels: the logical size times the window's scale, which is 1 on a plain X server.
    let scale = f64::from(info.width) / logical[2];
    assert!(scale >= 1.0 && (f64::from(info.height) - logical[3] * scale).abs() <= 1.0);
    assert_eq!(info.color_type, png::ColorType::Rgba);
    let first = &pixels[..4];
    assert!(
        pixels[..info.buffer_size()]
            .chunks(4)
            .any(|pixel| pixel != first),
        "the screenshot of the button is one colour"
    );
    Ok(())
}

/// A path that isn't there is an error the client reads on the next call, not a fault; everything
/// outside the protocol is a fault, not a call that quietly did nothing.
fn errors(port: u16) -> Result<(), String> {
    let before = errors_list(port)?.len();
    assert_eq!(
        call(
            port,
            "getStringProperty",
            &[path("mainWindow/nope"), property("text")]
        )?,
        Answer::Text(String::new())
    );
    let errors = errors_list(port)?;
    assert_eq!(
        errors.get(before).map(String::as_str),
        Some("GetProperty: Item not found: mainWindow/nope"),
        "{errors:?}"
    );
    assert_eq!(
        call(port, "takeScreenshotAsBase64", &[path("mainWindow")]),
        Err("unsupported".to_owned())
    );
    assert_eq!(call(port, "nonsense", &[]), Err("unsupported".to_owned()));
    Ok(())
}

/// Makes one call over the socket and reads what the server answered.
fn call(port: u16, method: &str, params: &[String]) -> Result<Answer, String> {
    let response = rpc(port, method, params)?;
    let (_, body) = response.split_once("\r\n\r\n").ok_or("no HTTP body")?;
    answer(body)
}

/// Sends one call and reads the answer: what an XML-RPC client over `ureq` does,
/// in the fewest lines that still look like the protocol.
fn rpc(port: u16, method: &str, params: &[String]) -> Result<String, String> {
    let mut parameters = String::new();
    for param in params {
        let _ = write!(parameters, "<param>{param}</param>");
    }
    let body = format!(
        r#"<?xml version="1.0"?><methodCall><methodName>{method}</methodName><params>{parameters}</params></methodCall>"#
    );
    let request = format!(
        "POST /RPC2 HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = TcpStream::connect(("127.0.0.1", port)).map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut stream, request.as_bytes())
        .map_err(|error| error.to_string())?;
    let mut response = String::new();
    std::io::Read::read_to_string(&mut stream, &mut response).map_err(|error| error.to_string())?;
    Ok(response)
}

/// Reads the value of a `methodResponse`, or the string of its fault.
fn answer(body: &str) -> Result<Answer, String> {
    let document = roxmltree::Document::parse(body).map_err(|error| error.to_string())?;
    let root = document.root_element();
    let value = root
        .descendants()
        .find(|node| node.has_tag_name("value"))
        .ok_or("no value in the answer")?;
    if root.descendants().any(|node| node.has_tag_name("fault")) {
        return Err(fault_string(&document).ok_or("a fault without a faultString")?);
    }
    let Some(typed) = value.children().find(roxmltree::Node::is_element) else {
        return Ok(Answer::Text(text_of(value)));
    };
    match typed.tag_name().name() {
        "boolean" => Ok(Answer::Bool(typed.text() == Some("1"))),
        "array" => Ok(Answer::Array(
            typed
                .descendants()
                .filter(|node| node.has_tag_name("value"))
                .map(text_of)
                .collect(),
        )),
        _ => Ok(Answer::Text(text_of(value))),
    }
}

/// The `faultString` of a fault response.
fn fault_string(document: &roxmltree::Document) -> Option<String> {
    let member = document.descendants().find(|node| {
        node.has_tag_name("member")
            && node
                .children()
                .any(|child| child.has_tag_name("name") && child.text() == Some("faultString"))
    })?;
    let value = member
        .descendants()
        .find(|node| node.has_tag_name("value"))?;
    Some(text_of(value))
}

/// The text of a `<value>`: its own text when it is untyped, which is how Spix writes its strings,
/// else the text of the type inside it (`<double>40</double>` and friends).
fn text_of(value: roxmltree::Node) -> String {
    match value.children().find(roxmltree::Node::is_element) {
        Some(typed) => typed.text().unwrap_or_default().to_owned(),
        None => value.text().unwrap_or_default().to_owned(),
    }
}

/// The `<value>` of one string parameter.
fn value(inner: &str) -> String {
    format!("<value>{inner}</value>")
}

fn path(path: &str) -> String {
    value(&format!("<string>{path}</string>"))
}

fn property(name: &str) -> String {
    value(&format!("<string>{name}</string>"))
}

/// A port nothing listens on right now.
fn free_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let address = listener.local_addr().map_err(|error| error.to_string())?;
    Ok(address.port())
}
