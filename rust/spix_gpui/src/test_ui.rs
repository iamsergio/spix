// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The backend's primitives against a window of their own, `tests/support/mod.rs`.
//!
//! GPUI runs one event loop per process, so this module holds the one test that starts an app, and
//! it runs every check inside it, as the foreground tasks the server would make of its calls. A
//! check that fails returns its message, which is asserted once the loop is out: a panic inside
//! GPUI's executor would not end the test.

#[path = "../tests/support/mod.rs"]
mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::input::InputState;
use gpui_kit::{AsyncApp, Entity};
use spix_server::{Backend, Button, Clicks, Modifier, Point, PointerEvent};

use crate::backend::GpuiBackend;
use crate::gui;
use crate::lookup::Item;

const WINDOW: &str = "mainWindow";

/// Whether the machine has a display: GPUI picks its headless platform without one, which draws
/// nothing.
fn has_pixels() -> bool {
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

fn item(backend: &GpuiBackend, path: &[&str]) -> Result<Item, String> {
    backend
        .find(WINDOW, path)
        .ok_or_else(|| format!("{path:?} is not there"))
}

fn text(backend: &GpuiBackend, path: &[&str]) -> Result<String, String> {
    let found = item(backend, path)?;
    backend
        .property(WINDOW, &found, "text")
        .ok_or_else(|| format!("{path:?} has no text"))
}

/// The status line the view shows, which says what has happened to it.
fn status(backend: &GpuiBackend) -> Result<String, String> {
    text(backend, &["status"])
}

fn expect(what: &str, got: &str, wanted: &str) -> Result<(), String> {
    if got.contains(wanted) {
        Ok(())
    } else {
        Err(format!("{what}: wanted {wanted:?} in {got:?}"))
    }
}

fn center(backend: &GpuiBackend, found: &Item) -> Result<Point, String> {
    let frame = backend
        .frame(WINDOW, found)
        .ok_or("the item has no frame")?;
    Ok(Point {
        x: frame.x + frame.width / 2.0,
        y: frame.y + frame.height / 2.0,
    })
}

async fn click(backend: &GpuiBackend, path: &[&str]) -> Result<(), String> {
    let found = item(backend, path)?;
    backend
        .click(WINDOW, &found, Button::Left, Clicks::Single)
        .await;
    Ok(())
}

fn lookup_and_properties(backend: &GpuiBackend) -> Result<(), String> {
    expect("the label", &text(backend, &["label"])?, "Hello")?;
    expect(
        "a path through ancestors",
        &text(backend, &["root", "label"])?,
        "Hello",
    )?;
    if backend.find(WINDOW, &["nothing"]).is_some() || backend.find("other", &["label"]).is_some() {
        return Err("a missing path was found".into());
    }
    let window = item(backend, &[])?;
    let frame = backend
        .frame(WINDOW, &window)
        .ok_or("the window has no frame")?;
    if frame.width <= 0.0 || !backend.visible(WINDOW, &window) {
        return Err(format!("the window is {frame:?}"));
    }

    let button = item(backend, &["toolbar", "button"])?;
    let frame = backend.frame(WINDOW, &button).ok_or("no frame")?;
    let on_screen = backend.bounds(WINDOW, &button).ok_or("no bounds")?;
    if frame.width <= 0.0 || (on_screen.width - frame.width).abs() > f64::EPSILON {
        return Err(format!("{frame:?} and {on_screen:?}"));
    }
    if !backend.visible(WINDOW, &button) {
        return Err("the button is not visible".into());
    }
    let enabled = |path: &[&str]| -> Result<String, String> {
        let found = item(backend, path)?;
        backend
            .property(WINDOW, &found, "enabled")
            .ok_or_else(|| "no enabled".to_owned())
    };
    expect("enabled", &enabled(&["button"])?, "true")?;
    expect("disabled", &enabled(&["off"])?, "false")?;

    let list = item(backend, &["list"])?;
    expect(
        "count, from the hook",
        &backend.property(WINDOW, &list, "count").unwrap_or_default(),
        &support::ROWS.to_string(),
    )?;
    if backend.property(WINDOW, &list, "nonsense").is_some() {
        return Err("an unknown property answered".into());
    }
    if backend.set_property(WINDOW, &list, "count", "1") {
        return Err("a property nothing can set was set".into());
    }
    Ok(())
}

async fn clicks(backend: &GpuiBackend) -> Result<(), String> {
    click(backend, &["button"]).await?;
    expect("one click", &status(backend)?, "clicks=1 ")?;

    let button = item(backend, &["button"])?;
    backend
        .click(WINDOW, &button, Button::Left, Clicks::Double)
        .await;
    let seen = status(backend)?;
    expect("a double click is two clicks", &seen, "clicks=3 ")?;
    expect("and counts as a double click", &seen, "double=1 ")?;

    backend.modifier(WINDOW, Modifier::Shift, true);
    click(backend, &["button"]).await?;
    backend.modifier(WINDOW, Modifier::Shift, false);
    expect("shift click", &status(backend)?, "shift=1 ")?;
    click(backend, &["button"]).await?;
    expect("shift let go", &status(backend)?, "shift=1 ")?;

    click(backend, &["check"]).await?;
    expect("checkbox", &status(backend)?, "checked=true")?;
    let check = item(backend, &["check"])?;
    expect(
        "the checked property",
        &backend
            .property(WINDOW, &check, "checked")
            .unwrap_or_default(),
        "true",
    )
}

async fn typing(backend: &GpuiBackend) -> Result<(), String> {
    click(backend, &["field"]).await?;
    backend.input_text(WINDOW, "hi ü");
    expect("typed text", &text(backend, &["field"])?, "hi ü")?;

    // Enter, by its Qt code: the input's own binding runs.
    backend.enter_key(WINDOW, 0x0100_0004, &[]);
    backend.enter_key(WINDOW, 0x0100_0003, &[]);
    // The subscription of the view hears the event once the effects are flushed.
    backend.sleep(Duration::from_millis(20)).await;
    expect("the enter key", &status(backend)?, "enters=1 ")?;
    expect("backspace", &text(backend, &["field"])?, "hi ")?;

    let field = item(backend, &["field"])?;
    if !backend.set_property(WINDOW, &field, "text", "set") {
        return Err("the registered input would not take its text".into());
    }
    expect("the set text", &text(backend, &["field"])?, "set")
}

/// The text of a masked input is what was set, not its placeholder, and empty while it is.
fn masked_input(backend: &GpuiBackend) -> Result<(), String> {
    expect("the empty secret", &text(backend, &["secretField"])?, "")?;
    let field = item(backend, &["secretField"])?;
    if !backend.set_property(WINDOW, &field, "text", "hunter2") {
        return Err("the masked input would not take its text".into());
    }
    expect("the secret", &text(backend, &["secretField"])?, "hunter2")?;
    if !backend.set_property(WINDOW, &field, "text", "") {
        return Err("the masked input would not be emptied".into());
    }
    expect("the emptied secret", &text(backend, &["secretField"])?, "")
}

/// Two inputs with one own id under two parents are told apart by their paths, and a path that
/// matches two registered inputs is refused.
fn scoped_inputs(backend: &GpuiBackend, spare: Entity<InputState>) -> Result<(), String> {
    let room = item(backend, &["roomComposer", "composerField"])?;
    let thread = item(backend, &["threadPane", "composerField"])?;
    if !backend.set_property(WINDOW, &room, "text", "room") {
        return Err("the room's input would not take its text".into());
    }
    expect(
        "the room's input",
        &text(backend, &["roomComposer", "composerField"])?,
        "room",
    )?;
    if !text(backend, &["threadPane", "composerField"])?.is_empty() {
        return Err("the thread's input changed with the room's".into());
    }
    if !backend.set_property(WINDOW, &thread, "text", "thread") {
        return Err("the thread's input would not take its text".into());
    }
    expect(
        "the thread's input",
        &text(backend, &["threadPane", "composerField"])?,
        "thread",
    )?;
    expect(
        "the room's input, still",
        &text(backend, &["roomComposer", "composerField"])?,
        "room",
    )?;

    // The bare id matches both inputs from now on: nothing is guessed.
    crate::register_input("composerField", spare);
    if backend.set_property(WINDOW, &room, "text", "again") {
        return Err("an ambiguous registration set a text".into());
    }
    expect(
        "the room's input, untouched",
        &text(backend, &["roomComposer", "composerField"])?,
        "room",
    )
}

fn wheel_and_drag(backend: &GpuiBackend) -> Result<(), String> {
    let list = item(backend, &["list"])?;
    let at = center(backend, &list)?;
    item(backend, &["list", "row-0"])?;
    for _ in 0..5 {
        backend.pointer(
            WINDOW,
            at,
            PointerEvent::Wheel {
                angle_x: 0,
                angle_y: -120,
            },
        );
    }
    if backend.find(WINDOW, &["row-0"]).is_some() {
        return Err("a negative wheel did not scroll the list towards its end".into());
    }
    item(backend, &["row-13"])?;
    for _ in 0..5 {
        backend.pointer(
            WINDOW,
            at,
            PointerEvent::Wheel {
                angle_x: 0,
                angle_y: 120,
            },
        );
    }
    item(backend, &["row-0"])?;

    let pad = item(backend, &["pad"])?;
    let from = center(backend, &pad)?;
    backend.pointer(WINDOW, from, PointerEvent::Press(Button::Left));
    let to = Point {
        x: from.x,
        y: from.y + 30.0,
    };
    backend.pointer(WINDOW, to, PointerEvent::Move);
    backend.pointer(WINDOW, to, PointerEvent::Release(Button::Left));
    expect(
        "a drag",
        &status(backend)?,
        &format!("dragged={:.0} ", to.y),
    )
}

async fn menu(backend: &GpuiBackend) -> Result<(), String> {
    if backend.find(WINDOW, &["menu-item"]).is_some() {
        return Err("a menu before anything opened it".into());
    }
    let anchor = item(backend, &["anchor"])?;
    backend
        .click(WINDOW, &anchor, Button::Right, Clicks::Single)
        .await;
    let found = item(backend, &["menu", "menu-item"])?;
    expect(
        "the popup's label",
        &text(backend, &["menu-item"])?,
        "Menu item",
    )?;
    if !backend.visible(WINDOW, &found) {
        return Err("the popup's item is not visible".into());
    }
    click(backend, &["menu-item"]).await?;
    expect("the popup's item", &status(backend)?, "menu=1 ")?;
    if backend.find(WINDOW, &["menu-item"]).is_some() {
        return Err("the menu is still open".into());
    }
    Ok(())
}

fn screenshot(backend: &GpuiBackend) -> Result<(), String> {
    if !has_pixels() {
        println!("skipped: no display, so GPUI's headless platform has no pixels");
        return match backend.window_image(WINDOW) {
            Err(_) => Ok(()),
            Ok(_) => Err("pixels without a display".into()),
        };
    }
    let image = backend.window_image(WINDOW)?;
    let size = u64::from(image.width) * u64::from(image.height) * 4;
    if usize::try_from(size) != Ok(image.rgba.len()) || image.scale <= 0.0 {
        return Err(format!(
            "{}x{} at {}",
            image.width, image.height, image.scale
        ));
    }
    if image
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .all(|pixel| pixel[..3] == image.rgba[..3])
    {
        return Err("the image is one colour".into());
    }
    Ok(())
}

async fn all(backend: GpuiBackend, spare: Entity<InputState>) -> Result<(), String> {
    lookup_and_properties(&backend)?;
    clicks(&backend).await?;
    typing(&backend).await?;
    masked_input(&backend)?;
    scoped_inputs(&backend, spare)?;
    wheel_and_drag(&backend)?;
    menu(&backend).await?;
    screenshot(&backend)?;
    Ok(())
}

#[test]
fn the_backend_drives_a_window() -> Result<(), String> {
    let outcome: Rc<RefCell<Option<Result<(), String>>>> = Rc::new(RefCell::new(None));
    let result = Rc::clone(&outcome);
    gpui_kit::platform::application().run(move |cx| {
        let opened = support::open(cx);
        let Ok((handle, view)) = opened else {
            *result.borrow_mut() = Some(opened.map(|_| ()));
            cx.quit();
            return;
        };
        let spare = view.read(cx).scoped[0].1.clone();
        let tasks = gui::attach(cx, [(WINDOW.to_owned(), handle)].into_iter());
        let backend = GpuiBackend::new(tasks);
        cx.activate(true);
        cx.spawn(async move |cx: &mut AsyncApp| {
            // Let the first frame be drawn before anything looks for an element.
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            let outcome = all(backend, spare).await;
            *result.borrow_mut() = Some(outcome);
            cx.update(|app| app.quit());
        })
        .detach();
    });
    outcome
        .borrow_mut()
        .take()
        .unwrap_or_else(|| Err("the app ended without running the checks".to_owned()))
}
