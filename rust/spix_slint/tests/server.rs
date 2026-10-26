// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The server over a real socket: one call of every method the server speaks, the faults for
//! what the protocol doesn't speak, and `quit` ending the event loop the way it ends the app's.
//! The testing backend can be set once per process, so this binary holds one test.

use std::fmt::Write as _;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use i_slint_backend_testing::TestingBackend;
use i_slint_backend_testing::testing_backend::TestingBackendOptions;
use slint::ComponentHandle;

slint::slint! {
    import { ListView } from "std-widgets.slint";

    export component ServerTestWindow inherits Window {
        in-out property <int> clicks;
        in-out property <int> menu-clicks;
        in-out property <string> field-text <=> field.text;

        width: 300px;
        height: 200px;

        Text {
            accessible-role: text;
            accessible-id: "label";
            text: "Hello";
        }

        Rectangle {
            accessible-role: button;
            accessible-id: "button";
            accessible-label: "Press";
            accessible-enabled: true;
            accessible-checked: root.clicks > 0;
            x: 0;
            y: 60px;
            width: 80px;
            height: 40px;

            TouchArea {
                clicked => {
                    root.clicks += 1;
                }
            }
        }

        field := TextInput {
            accessible-id: "field";
            x: 100px;
            y: 60px;
            width: 80px;
            height: 30px;
        }

        // Taller than it shows, to be scrolled by the wheel.
        list := ListView {
            accessible-id: "scroller";
            x: 200px;
            y: 100px;
            width: 90px;
            height: 80px;

            for index in 20: Rectangle {
                height: 20px;
            }
        }

        // How far the list has scrolled towards its end, in pixels.
        Rectangle {
            accessible-role: button;
            accessible-id: "scrolled";
            accessible-value: Math.round(-list.content-y / 1px);
            x: 100px;
            y: 0;
            width: 10px;
            height: 10px;
        }

        // A known colour and size, for `takeScreenshot`.
        Rectangle {
            accessible-role: button;
            accessible-id: "swatch";
            background: #3366cc;
            x: 200px;
            y: 60px;
            width: 50px;
            height: 30px;
        }

        anchor := Rectangle {
            accessible-role: button;
            accessible-id: "anchor";
            x: 0;
            y: 130px;
            width: 120px;
            height: 40px;

            TouchArea {
                clicked => {
                    menu.show();
                }
                pointer-event(event) => {
                    if event.button == PointerEventButton.right && event.kind == PointerEventKind.up {
                        menu.show();
                    }
                }
            }

            menu := PopupWindow {
                x: 40px;
                y: 40px;
                width: 140px;
                height: 70px;

                Rectangle {
                    accessible-role: button;
                    accessible-id: "menu-item";
                    accessible-label: "Menu item";
                    accessible-item-count: root.menu-clicks;
                    x: 0;
                    y: 0;
                    width: 140px;
                    height: 30px;

                    TouchArea {
                        clicked => {
                            root.menu-clicks += 1;
                            menu.close();
                        }
                    }
                }
            }
        }
    }
}

/// What one call answered: the text of its value, its boolean, or the entries of an array read as
/// text (bounding boxes come as numbers, `getErrors` as strings).
#[derive(Debug, PartialEq)]
enum Answer {
    Text(String),
    Bool(bool),
    Array(Vec<String>),
}

/// Asks the app to quit when the client thread ends, however it ends: the event loop on the main
/// thread waits for it, and a client that gave up would otherwise leave it waiting forever.
struct QuitWhenDone;

impl Drop for QuitWhenDone {
    fn drop(&mut self) {
        let _ = slint::quit_event_loop();
    }
}

// INFRA-004
#[test]
fn answers_the_calls_of_the_gui_tests_and_quits() -> Result<(), String> {
    // Real time, as the app runs under the tests: timers and the core's events keep working. The
    // software renderer, because the mock one has no pixels for `takeScreenshot`.
    slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions {
        mock_time: false,
        threading: true,
        renderer_name: Some("software".into()),
    })))
    .map_err(|error| error.to_string())?;

    let window = ServerTestWindow::new().map_err(|error| error.to_string())?;
    let port = free_port()?;
    spix_slint::start(port, [("mainWindow", window.as_weak())])
        .map_err(|error| error.to_string())?;

    // The calls run while the event loop below does: the server hands every one of them to it.
    let client = thread::spawn(move || drive(port));
    let ran = window.run().map_err(|error| error.to_string());
    let answered = client
        .join()
        .map_err(|_| "the client thread panicked".to_owned());
    ran?;
    answered??;

    assert_eq!(
        window.get_clicks(),
        3,
        "the click, then the two of the double click"
    );
    Ok(())
}

/// Makes every call of the protocol against `port`, in the order the tests make them.
fn drive(port: u16) -> Result<(), String> {
    let _quit = QuitWhenDone;

    existence(port)?;
    properties(port)?;
    clicks(port)?;
    text(port)?;
    keys(port)?;
    waiting(port)?;
    wheel(port)?;
    errors(port)?;
    screenshots(port)?;

    menu(port)?;

    assert_eq!(call(port, "quit", &[])?, Answer::Text(String::new()));
    Ok(())
}

/// A menu, which the scene holds in a `PopupWindow`: the click on its anchor opens it, the click
/// on its item reaches into the popup and runs the item's own `clicked`, and a right click opens
/// it again, which is how the tests open the menus of an app.
fn menu(port: u16) -> Result<(), String> {
    assert_eq!(
        call(port, "existsAndVisible", &[path("mainWindow/menu-item")])?,
        Answer::Bool(false),
        "no menu before anything clicked it"
    );

    call(port, "mouseClick", &[path("mainWindow/anchor")])?;
    assert_eq!(
        call(port, "existsAndVisible", &[path("mainWindow/menu-item")])?,
        Answer::Bool(true),
        "a click on the anchor opens the menu"
    );
    assert_eq!(count(port, "mainWindow/menu-item")?, "0");

    call(port, "mouseClick", &[path("mainWindow/menu-item")])?;
    assert_eq!(
        call(port, "existsAndVisible", &[path("mainWindow/menu-item")])?,
        Answer::Bool(false),
        "the click on the item ran its clicked, which closed the menu as an app's does"
    );

    call(
        port,
        "mouseClickWithButton",
        &[
            path("mainWindow/anchor"),
            value("<int>2</int>"),
            value("<int>0</int>"),
        ],
    )?;
    assert_eq!(
        call(port, "existsAndVisible", &[path("mainWindow/menu-item")])?,
        Answer::Bool(true),
        "a right click opens the menu too"
    );
    assert_eq!(
        count(port, "mainWindow/menu-item")?,
        "1",
        "the click on the item ran once"
    );
    Ok(())
}

/// What an element says its `count` is.
fn count(port: u16, at: &str) -> Result<String, String> {
    match call(port, "getStringProperty", &[path(at), property("count")])? {
        Answer::Text(count) => Ok(count),
        other => Err(format!("the count of {at} is {other:?}")),
    }
}

/// What is there, what isn't, and the window itself.
fn existence(port: u16) -> Result<(), String> {
    assert_eq!(
        call(port, "existsAndVisible", &[path("mainWindow/label")])?,
        Answer::Bool(true)
    );
    assert_eq!(
        call(port, "existsAndVisible", &[path("mainWindow/nothing")])?,
        Answer::Bool(false)
    );
    assert_eq!(
        call(port, "existsAndVisible", &[path("mainWindow")])?,
        Answer::Bool(true)
    );
    Ok(())
}

/// What the tests read from an element, and where it stands on the screen.
fn properties(port: u16) -> Result<(), String> {
    assert_eq!(
        call(
            port,
            "getStringProperty",
            &[path("mainWindow/label"), property("text")]
        )?,
        Answer::Text("Hello".to_owned())
    );

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

/// A click through the whole way: it reaches the `TouchArea` behind the button, which shows in the
/// property the tests read.
fn clicks(port: u16) -> Result<(), String> {
    call(port, "mouseClick", &[path("mainWindow/button")])?;
    assert_eq!(
        call(
            port,
            "getStringProperty",
            &[path("mainWindow/button"), property("checked")]
        )?,
        Answer::Text("true".to_owned())
    );
    Ok(())
}

/// Text: a click focuses the field, the keys go to it, and a set property is read back.
fn text(port: u16) -> Result<(), String> {
    call(
        port,
        "mouseClickWithButton",
        &[
            path("mainWindow/field"),
            value("<int>1</int>"),
            value("<int>0</int>"),
        ],
    )?;
    call(
        port,
        "inputText",
        &[path("mainWindow/field"), value("<string>hi ü</string>")],
    )?;
    assert_eq!(
        call(
            port,
            "getStringProperty",
            &[path("mainWindow/field"), property("text")]
        )?,
        Answer::Text("hi ü".to_owned())
    );
    call(
        port,
        "setStringProperty",
        &[
            path("mainWindow/field"),
            property("text"),
            value("<string>set</string>"),
        ],
    )?;
    assert_eq!(
        call(
            port,
            "getStringProperty",
            &[path("mainWindow/field"), property("text")]
        )?,
        Answer::Text("set".to_owned())
    );
    Ok(())
}

/// The return key, through its Spix (Qt) key code, and a double click on the same button. How many
/// clicks landed is read on the main thread once the loop is out, since the client can only ask an
/// element for it.
fn keys(port: u16) -> Result<(), String> {
    call(
        port,
        "enterKey",
        &[
            path("mainWindow/field"),
            value("<int>16777220</int>"),
            value("<int>0</int>"),
        ],
    )?;
    call(port, "mouseDoubleClick", &[path("mainWindow/button")])?;
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

/// How far the list has scrolled towards its end.
fn scrolled(port: u16) -> Result<i32, String> {
    text_of_item(port, "mainWindow/scrolled")?
        .parse()
        .map_err(|error| format!("the scroll position is no number: {error}"))
}

/// One call of the wheel: `steps` turns of `angle_y` over the list, `interval` milliseconds apart.
fn wheel_call(port: u16, angle_y: i32, steps: i32, interval: i32) -> Result<Answer, String> {
    call(
        port,
        "mouseWheel",
        &[
            path("mainWindow/scroller"),
            value("<int>0</int>"),
            value(&format!("<int>{angle_y}</int>")),
            value(&format!("<int>{steps}</int>")),
            value(&format!("<int>{interval}</int>")),
        ],
    )
}

/// The wheel scrolls a list: a negative y moves the content towards its end and a positive one
/// back, more steps go further, and the steps are spread over the interval between them.
fn wheel(port: u16) -> Result<(), String> {
    assert_eq!(scrolled(port)?, 0);

    wheel_call(port, -120, 1, 0)?;
    let one = scrolled(port)?;
    assert!(one > 0, "a negative y scrolls towards the end: {one}");

    wheel_call(port, 120, 1, 0)?;
    assert_eq!(scrolled(port)?, 0, "a positive y scrolls back to the start");

    let started = Instant::now();
    wheel_call(port, -120, 3, 40)?;
    let three = scrolled(port)?;
    assert!(
        three > one,
        "three steps ({three}) go further than one ({one})"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(80),
        "the call returned after {:?}, before the two intervals were up",
        started.elapsed()
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
    assert_eq!(scrolled(port)?, three, "a refused call scrolls nothing");

    call(
        port,
        "mouseWheel",
        &[
            path("mainWindow/nope"),
            value("<int>0</int>"),
            value("<int>-120</int>"),
            value("<int>2</int>"),
            value("<int>10</int>"),
        ],
    )?;
    assert_eq!(
        call(port, "getErrors", &[])?,
        Answer::Array(vec!["Wheel: Item not found: mainWindow/nope".to_owned()])
    );
    Ok(())
}

/// A screenshot of an element is a PNG of its size and colour, written before the call answers.
fn screenshots(port: u16) -> Result<(), String> {
    let file = std::env::temp_dir().join(format!("spix_slint_swatch_{}.png", std::process::id()));
    let file_name = file.to_string_lossy().into_owned();
    assert_eq!(
        call(
            port,
            "takeScreenshot",
            &[path("mainWindow/swatch"), path(&file_name)]
        )?,
        Answer::Text(String::new())
    );
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&file).map_err(|error| error.to_string())?,
    ));
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("no buffer size")?];
    let info = reader
        .next_frame(&mut pixels)
        .map_err(|error| error.to_string())?;
    let _ = std::fs::remove_file(&file);
    assert_eq!((info.width, info.height), (50, 30));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    for pixel in pixels[..info.buffer_size()].chunks(4) {
        assert_eq!(pixel, [0x33, 0x66, 0xcc, 0xff]);
    }

    Ok(())
}

/// A path that isn't there is an error the client reads on the next call, not a fault; everything
/// outside the protocol is a fault, not a call that quietly did nothing. The wheel's own missing
/// path, from earlier, is the first entry.
fn errors(port: u16) -> Result<(), String> {
    assert_eq!(
        call(
            port,
            "getStringProperty",
            &[path("mainWindow/nope"), property("text")]
        )?,
        Answer::Text(String::new())
    );
    assert_eq!(
        call(port, "getErrors", &[])?,
        Answer::Array(vec![
            "Wheel: Item not found: mainWindow/nope".to_owned(),
            "GetProperty: Item not found: mainWindow/nope".to_owned()
        ])
    );
    // Like the other calls, one of a path that isn't there is an error in `getErrors`, not a fault.
    assert_eq!(
        call(
            port,
            "takeScreenshot",
            &[path("mainWindow/nope"), path("unused.png")]
        )?,
        Answer::Text(String::new())
    );
    assert_eq!(
        call(port, "getErrors", &[])?,
        Answer::Array(vec![
            "Wheel: Item not found: mainWindow/nope".to_owned(),
            "GetProperty: Item not found: mainWindow/nope".to_owned(),
            "TakeScreenshot: Item not found: mainWindow/nope".to_owned()
        ])
    );
    assert_eq!(
        call(port, "takeScreenshotAsBase64", &[path("mainWindow")]),
        Err("unsupported".to_owned())
    );
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
