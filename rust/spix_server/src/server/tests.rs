// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The protocol over the fake backend: faults, `getErrors`, the parameters, and what each gesture
//! becomes as primitive events and pauses.

use super::*;
use crate::WindowImage;
use crate::fake::{BUTTON, Fake};

fn call(method: &str, params: Vec<Value>) -> Call {
    Call {
        method: method.to_owned(),
        params,
    }
}

fn text(text: &str) -> Value {
    Value::String(text.to_owned())
}

fn path_param(path: &str) -> Vec<Value> {
    vec![text(path)]
}

fn serving(fake: Fake) -> Arc<Server<Fake>> {
    Arc::new(Server::new(fake))
}

/// A server over a window `main` with a button in it.
fn with_button() -> Arc<Server<Fake>> {
    serving(Fake::with(&[("button", BUTTON)]))
}

fn run(server: &Arc<Server<Fake>>, method: &str, params: Vec<Value>) -> Result<Value, Fault> {
    server.dispatch(call(method, params))
}

#[test]
fn reports_a_path_that_is_not_there_the_way_spix_does() {
    let server = serving(Fake::default());
    let read = run(
        &server,
        "getStringProperty",
        vec![text("mainWindow/nope"), text("text")],
    )
    .unwrap();
    assert_eq!(read, Value::String(String::new()));
    assert_eq!(
        server.errors(),
        vec!["GetProperty: Item not found: mainWindow/nope".to_owned()]
    );

    let bounds = run(&server, "getBoundingBox", path_param("elsewhere/item")).unwrap();
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
        server.errors(),
        vec![
            "GetProperty: Item not found: mainWindow/nope".to_owned(),
            "GetBoundingBox: Item not found: elsewhere/item".to_owned()
        ]
    );
}

#[test]
fn a_missing_path_is_not_an_error_for_exists_and_visible() {
    let server = with_button();
    let answer = run(&server, "existsAndVisible", path_param("main/nothing")).unwrap();
    assert_eq!(answer, Value::Bool(false));
    assert!(server.errors().is_empty(), "{:?}", server.errors());

    assert_eq!(
        run(&server, "existsAndVisible", path_param("main/button")).unwrap(),
        Value::Bool(true)
    );
    // The backend decides about visibility, and the window name alone is a path too.
    let hidden = Fake::with(&[("invisible", BUTTON)]);
    assert_eq!(
        run(
            &serving(hidden),
            "existsAndVisible",
            path_param("main/invisible")
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        run(&server, "existsAndVisible", path_param("main")).unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn clicks_and_keys_on_a_path_that_is_not_there_leave_the_spix_error() {
    let server = serving(Fake::default());
    run(&server, "mouseClick", path_param("mainWindow/nowhere")).unwrap();
    run(
        &server,
        "mouseClickWithButton",
        vec![text("mainWindow/nowhere"), Value::Int(1), Value::Int(0)],
    )
    .unwrap();
    run(
        &server,
        "mouseDoubleClick",
        path_param("mainWindow/nowhere"),
    )
    .unwrap();
    run(
        &server,
        "enterKey",
        vec![
            text("mainWindow/nowhere"),
            Value::Int(0x0100_0004),
            Value::Int(0),
        ],
    )
    .unwrap();
    run(
        &server,
        "inputText",
        vec![text("mainWindow/nowhere"), text("hello")],
    )
    .unwrap();
    assert_eq!(
        server.errors(),
        vec![
            "ClickOnItem: Item not found: mainWindow/nowhere".to_owned(),
            "ClickOnItem: Item not found: mainWindow/nowhere".to_owned(),
            "DoubleClickOnItem: Item not found: mainWindow/nowhere".to_owned(),
            "EnterKey: Item not found: mainWindow/nowhere".to_owned(),
            "InputText: Item not found: mainWindow/nowhere".to_owned(),
        ]
    );
}

#[test]
fn a_screenshot_of_a_path_that_is_not_there_leaves_the_spix_error() {
    let server = serving(Fake::default());
    run(
        &server,
        "takeScreenshot",
        vec![text("mainWindow/nowhere"), text("unused.png")],
    )
    .unwrap();
    assert_eq!(
        server.errors(),
        vec!["TakeScreenshot: Item not found: mainWindow/nowhere".to_owned()]
    );
}

#[test]
fn a_screenshot_is_cropped_to_the_item_and_written_before_the_call_answers() {
    // A 200x100 window at scale 1 whose pixels are all one colour.
    let mut fake = Fake::with(&[("button", BUTTON)]);
    fake.image = Some(WindowImage {
        width: 200,
        height: 100,
        scale: 1.0,
        rgba: [10, 20, 30, 255].repeat(200 * 100),
    });
    let server = serving(fake);
    let directory = std::env::temp_dir().join(format!("spix_server_{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("button.png");
    run(
        &server,
        "takeScreenshot",
        vec![text("main/button"), text(file.to_str().unwrap())],
    )
    .unwrap();
    assert!(server.errors().is_empty(), "{:?}", server.errors());
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&file).unwrap()));
    let info = decoder.read_info().unwrap().info().clone();
    assert_eq!((info.width, info.height), (40, 10));
    std::fs::remove_dir_all(&directory).unwrap();

    // A backend without pixels says why, in `getErrors`.
    let server = with_button();
    run(
        &server,
        "takeScreenshot",
        vec![text("main/button"), text("unused.png")],
    )
    .unwrap();
    assert_eq!(
        server.errors(),
        vec!["TakeScreenshot: main/button: the window can't be rendered: no pixels".to_owned()]
    );
}

#[test]
fn get_errors_keeps_what_it_has_reported_so_far() {
    let server = serving(Fake::default());
    run(&server, "mouseClick", path_param("mainWindow/nowhere")).unwrap();
    let reported = run(&server, "getErrors", vec![]).unwrap();
    assert_eq!(
        reported,
        Value::Array(vec![Value::String(
            "ClickOnItem: Item not found: mainWindow/nowhere".to_owned()
        )])
    );
}

#[test]
fn answers_a_fault_for_everything_outside_the_protocol() {
    let server = with_button();
    // `takeScreenshotAsBase64` is the C++ Spix call this server doesn't have.
    let unknown = run(&server, "takeScreenshotAsBase64", vec![text("a")]).unwrap_err();
    assert_eq!(unknown.code, FAULT_UNSUPPORTED);
    assert_eq!(unknown.message, "unsupported");

    let malformed = server.process("not xml at all").unwrap_err();
    assert_eq!(malformed.code, FAULT_INVALID_REQUEST);

    let array = server.process("<?xml version=\"1.0\"?><methodCall><methodName>m</methodName><params><param><value><array><data></data></array></value></param></params></methodCall>").unwrap_err();
    assert_eq!(array.code, FAULT_UNSUPPORTED);
    assert_eq!(array.message, "unsupported");

    // A parameter of the wrong type, and a button that doesn't exist.
    let wrong = run(&server, "mouseClick", vec![Value::Int(1)]).unwrap_err();
    assert_eq!(wrong.code, FAULT_UNSUPPORTED);
    let button = run(
        &server,
        "mouseClickWithButton",
        vec![text("main/button"), Value::Int(3), Value::Int(0)],
    )
    .unwrap_err();
    assert_eq!(button.code, FAULT_UNSUPPORTED);
    assert!(server.backend.log().is_empty());
}

#[test]
fn reads_the_body_of_the_request_ureq_sends() {
    let request = "POST /RPC2 HTTP/1.1\r\nHost: 127.0.0.1:80\r\nContent-Type: text/xml\r\nContent-Length: 7\r\nConnection: keep-alive\r\n\r\n<ping/>";
    let body = read_request(&mut request.as_bytes()).unwrap();
    assert_eq!(body, "<ping/>");
}

#[test]
fn splits_a_path_into_the_window_and_the_ids_below_it() {
    assert_eq!(split_path("main"), Some(("main", vec![])));
    assert_eq!(split_path("main/a/b"), Some(("main", vec!["a", "b"])));
    assert_eq!(split_path(""), None);
    assert_eq!(split_path("/a"), None);
}

#[test]
fn a_whole_request_goes_through_the_codec_to_the_backend() {
    let server = with_button();
    let request = "<?xml version=\"1.0\"?><methodCall><methodName>getBoundingBox</methodName><params><param><value><string>main/button</string></value></param></params></methodCall>";
    let answer = server.process(request).unwrap();
    // Screen coordinates, where the fake puts its window at (1000, 500).
    assert_eq!(
        answer,
        Value::Array(vec![
            Value::Double(1010.0),
            Value::Double(520.0),
            Value::Double(40.0),
            Value::Double(10.0)
        ])
    );
}

#[test]
fn quit_is_the_backends_and_wait_sleeps_on_the_calling_thread() {
    let server = with_button();
    let started = Instant::now();
    server
        .process("<?xml version=\"1.0\"?><methodCall><methodName>wait</methodName><params><param><value><int>20</int></value></param></params></methodCall>")
        .unwrap();
    assert!(started.elapsed() >= Duration::from_millis(20));
    // `wait` never reached the GUI thread: the fake would have logged a sleep.
    assert!(server.backend.log().is_empty());

    server
        .process("<?xml version=\"1.0\"?><methodCall><methodName>quit</methodName><params></params></methodCall>")
        .unwrap();
    assert_eq!(server.backend.log(), vec!["quit".to_owned()]);
}

#[test]
fn text_and_keys_go_to_the_window_with_the_modifiers_decoded() {
    let server = with_button();
    run(&server, "inputText", vec![text("main/button"), text("hi")]).unwrap();
    run(
        &server,
        "enterKey",
        vec![text("main/button"), Value::Int(0x56), Value::Int(1 | 4)],
    )
    .unwrap();
    assert_eq!(
        server.backend.log(),
        vec![
            "text \"hi\" to main".to_owned(),
            "key 0x56 [Shift, Alt] to main".to_owned()
        ]
    );
}

#[test]
fn a_property_that_cannot_be_set_is_reported_and_one_that_can_is_read_back() {
    let server = with_button();
    run(
        &server,
        "setStringProperty",
        vec![text("main/button"), text("text"), text("abc")],
    )
    .unwrap();
    assert_eq!(
        run(
            &server,
            "getStringProperty",
            vec![text("main/button"), text("text")]
        )
        .unwrap(),
        text("abc")
    );
    assert!(server.errors().is_empty());

    run(
        &server,
        "setStringProperty",
        vec![text("main/button"), text("checked"), text("true")],
    )
    .unwrap();
    assert_eq!(
        server.errors(),
        vec!["setStringProperty: property 'checked' is not settable on a Fake element".to_owned()]
    );
}

#[test]
fn a_click_presses_and_releases_at_the_centre_with_the_modifiers_around_it() {
    let server = with_button();
    run(&server, "mouseClick", path_param("main/button")).unwrap();
    run(
        &server,
        "mouseClickWithButton",
        vec![text("main/button"), Value::Int(2), Value::Int(1 | 2)],
    )
    .unwrap();
    assert_eq!(
        server.backend.log(),
        vec![
            "press Left at 30,25",
            "release Left at 30,25",
            "Shift down",
            "Control down",
            "press Right at 30,25",
            "release Right at 30,25",
            "Control up",
            "Shift up",
        ]
    );
}

#[test]
fn a_double_click_is_two_clicks_and_the_window_itself_cannot_be_clicked() {
    let server = with_button();
    run(&server, "mouseDoubleClick", path_param("main/button")).unwrap();
    assert_eq!(
        server.backend.log(),
        vec![
            "press Left at 30,25",
            "release Left at 30,25",
            "press Left at 30,25",
            "release Left at 30,25",
        ]
    );

    run(&server, "mouseClick", path_param("main")).unwrap();
    run(&server, "mouseDoubleClick", path_param("main")).unwrap();
    assert_eq!(
        server.errors(),
        vec![
            "ClickOnItem: not supported on the window itself: main".to_owned(),
            "DoubleClickOnItem: not supported on the window itself: main".to_owned()
        ]
    );
    assert_eq!(server.backend.log().len(), 4);
}

#[test]
fn the_wheel_turns_over_the_centre_with_the_interval_between_the_turns() {
    let server = with_button();
    run(
        &server,
        "mouseWheel",
        vec![
            text("main/button"),
            Value::Int(0),
            Value::Int(-120),
            Value::Int(3),
            Value::Int(2),
        ],
    )
    .unwrap();
    assert_eq!(
        server.backend.log(),
        vec![
            "wheel 0,-120 at 30,25",
            "sleep 2",
            "wheel 0,-120 at 30,25",
            "sleep 2",
            "wheel 0,-120 at 30,25",
        ]
    );

    // No steps, or a negative interval, are numbers the server refuses.
    for (steps, interval) in [(0, 0), (1, -1)] {
        let refused = run(
            &server,
            "mouseWheel",
            vec![
                text("main/button"),
                Value::Int(0),
                Value::Int(1),
                Value::Int(steps),
                Value::Int(interval),
            ],
        )
        .unwrap_err();
        assert_eq!(refused.code, FAULT_UNSUPPORTED);
    }
}

#[test]
fn a_drag_presses_moves_in_equal_parts_holds_and_releases() {
    let server = with_button();
    run(
        &server,
        "mouseDrag",
        vec![
            text("main/button"),
            Value::Int(2),
            Value::Double(4.0),
            Value::Int(10),
            Value::Int(4),
            Value::Int(2),
            Value::Int(5),
            Value::Int(5),
        ],
    )
    .unwrap();
    let log: Vec<String> = server
        .backend
        .log()
        .into_iter()
        .filter(|line| !line.starts_with("sleep"))
        .collect();
    // The button's top-left is (10, 20): the points are from there.
    assert_eq!(
        log,
        vec![
            "press Left at 12,24",
            "move at 16,24",
            "move at 20,24",
            "release Left at 20,24",
        ]
    );

    let refused = run(
        &server,
        "mouseDrag",
        vec![
            text("main/button"),
            Value::Int(0),
            Value::Int(0),
            Value::Int(1),
            Value::Int(1),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
        ],
    )
    .unwrap_err();
    assert_eq!(refused.code, FAULT_UNSUPPORTED);
}
