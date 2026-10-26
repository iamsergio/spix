// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The Spix input that goes through the window: `inputText` and `enterKey` dispatch key events to
//! it, which lands on the element that has the focus, and the modifier keys are pressed and
//! released around a click (pointer events carry none, so Slint tracks the modifiers from key
//! events). Which key a Spix key code becomes is the mapping the C++ backend found.

use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{LogicalPosition, SharedString, Window};

/// Logical pixels Slint scrolls for one notch of a mouse wheel: its winit backend multiplies a
/// `MouseScrollDelta::LineDelta` by 60 (`i-slint-backend-winit` 1.18.1, `winitwindowadapter.rs`,
/// the `WindowEvent::MouseWheel` arm).
const PIXELS_PER_NOTCH: f32 = 60.0;
/// Qt's `angleDelta` of one notch: eighths of a degree, 15 degrees.
const ANGLE_PER_NOTCH: f32 = 120.0;

/// Turns the wheel at `position` once by `angle_x` and `angle_y`, in Qt's `angleDelta` units. A
/// positive y scrolls the content towards its start, and so does a positive Slint delta.
#[allow(clippy::cast_precision_loss)] // Wheel deltas are small.
pub(crate) fn wheel(window: &Window, position: LogicalPosition, angle_x: i32, angle_y: i32) {
    let to_pixels = |angle: i32| angle as f32 / ANGLE_PER_NOTCH * PIXELS_PER_NOTCH;
    window.dispatch_event(WindowEvent::PointerScrolled {
        position,
        delta_x: to_pixels(angle_x),
        delta_y: to_pixels(angle_y),
    });
}

/// A Spix mouse button: 1 = left, 2 = right, 4 = middle. None of them is no click.
pub(crate) fn button(code: i32) -> Option<PointerEventButton> {
    match code {
        1 => Some(PointerEventButton::Left),
        2 => Some(PointerEventButton::Right),
        4 => Some(PointerEventButton::Middle),
        _ => None,
    }
}

/// Types `text`: one key event per code point, as a keyboard would send them, to whichever
/// element of the window has the focus.
pub(crate) fn input_text(window: &Window, text: &str) {
    for character in text.chars() {
        let key = SharedString::from(character.to_string());
        window.dispatch_event(WindowEvent::KeyPressed { text: key.clone() });
        window.dispatch_event(WindowEvent::KeyReleased { text: key });
    }
}

/// Presses and releases the key with the Spix key code `key_code` (Qt's `Qt::Key`), holding
/// `modifiers` around it. False when Slint has no key for that code: Spix then reports nothing.
pub(crate) fn enter_key(window: &Window, key_code: i32, modifiers: i32) -> bool {
    let Some(key) = key_text(key_code, modifiers) else {
        return false;
    };
    let modifiers = modifier_keys(modifiers);
    for modifier in modifiers.iter().copied() {
        press(window, modifier);
    }
    window.dispatch_event(WindowEvent::KeyPressed { text: key.clone() });
    window.dispatch_event(WindowEvent::KeyReleased { text: key });
    for modifier in modifiers.iter().rev().copied() {
        release(window, modifier);
    }
    true
}

/// Presses `key` down, which is how a modifier is held for a click or a typed key.
pub(crate) fn press(window: &Window, key: Key) {
    window.dispatch_event(WindowEvent::KeyPressed { text: key.into() });
}

/// Lets `key` up again, after the click or key it was held for.
pub(crate) fn release(window: &Window, key: Key) {
    window.dispatch_event(WindowEvent::KeyReleased { text: key.into() });
}

/// The modifier keys of `modifiers`, in the order they are pressed (and released backwards).
pub(crate) fn modifier_keys(modifiers: i32) -> Vec<Key> {
    let mut keys = Vec::new();
    for (flag, key) in [
        (1, Key::Shift),
        (2, Key::Control),
        (4, Key::Alt),
        (8, Key::Meta),
    ] {
        if modifiers & flag != 0 {
            keys.push(key);
        }
    }
    keys
}

/// The key Slint expects for a Spix key code, or `None` for a code Slint has no key for.
fn key_text(key_code: i32, modifiers: i32) -> Option<SharedString> {
    let special = match key_code {
        0x0100_0000 => Some(Key::Escape),
        0x0100_0001 => Some(Key::Tab),
        0x0100_0002 => Some(Key::Backtab),
        0x0100_0003 => Some(Key::Backspace),
        0x0100_0004 | 0x0100_0005 => Some(Key::Return),
        0x0100_0006 => Some(Key::Insert),
        0x0100_0007 => Some(Key::Delete),
        0x0100_0008 => Some(Key::Pause),
        0x0100_000a => Some(Key::SysReq),
        0x0100_0010 => Some(Key::Home),
        0x0100_0011 => Some(Key::End),
        0x0100_0012 => Some(Key::LeftArrow),
        0x0100_0013 => Some(Key::UpArrow),
        0x0100_0014 => Some(Key::RightArrow),
        0x0100_0015 => Some(Key::DownArrow),
        0x0100_0016 => Some(Key::PageUp),
        0x0100_0017 => Some(Key::PageDown),
        0x0100_0020 => Some(Key::Shift),
        0x0100_0021 => Some(Key::Control),
        0x0100_0022 => Some(Key::Meta),
        0x0100_0023 => Some(Key::Alt),
        0x0100_0024 => Some(Key::CapsLock),
        0x0100_0026 => Some(Key::ScrollLock),
        0x0100_0055 => Some(Key::Menu),
        _ => None,
    };
    if let Some(key) = special {
        return Some(key.into());
    }

    // The function keys are consecutive in Slint's private use area, F1 = U+F704.
    if (0x0100_0030..=0x0100_0047).contains(&key_code) {
        let offset = u32::try_from(key_code - 0x0100_0030).ok()?;
        return char::from_u32(0xf704 + offset).map(|key| SharedString::from(key.to_string()));
    }

    // Printable ASCII: Qt's key codes for letters are the upper case letters, and Slint wants the
    // key as it is typed, which is lower case without Shift.
    if (0x20..=0x7e).contains(&key_code) {
        let mut key = char::from_u32(u32::try_from(key_code).ok()?)?;
        if key.is_ascii_uppercase() && modifiers & 1 == 0 {
            key = key.to_ascii_lowercase();
        }
        return Some(SharedString::from(key.to_string()));
    }
    None
}

#[cfg(test)]
mod tests {
    use slint::ComponentHandle;

    use super::*;
    use crate::test_ui;

    // INFRA-004
    #[test]
    fn typed_text_reaches_the_field_that_has_the_focus() {
        let ui = test_ui::window();
        let window = ui.window();
        // A field takes the focus, as it does from the user: here from the test's own call, since
        // clicking is Slint's own `single_click`, which needs the event loop (see `tests/server.rs`).
        ui.invoke_focus_field();
        input_text(window, "Hello ü");
        assert_eq!(ui.get_field_text(), "Hello ü");
    }

    // INFRA-004
    #[test]
    fn the_return_key_accepts_the_field() {
        let ui = test_ui::window();
        let window = ui.window();
        ui.invoke_focus_field();

        assert!(enter_key(window, 0x0100_0004, 0));
        assert_eq!(ui.get_accepted(), "accepted");
    }

    // INFRA-004
    #[test]
    fn keys_map_from_the_spix_codes_to_the_slint_text() {
        // Qt's key code for Return, and the letters as Qt has them, lower case without Shift.
        assert_eq!(key_text(0x0100_0004, 0), Some(Key::Return.into()));
        assert_eq!(key_text(0x56, 0), Some("v".into()));
        assert_eq!(key_text(0x56, 1), Some("V".into()));
        assert_eq!(key_text(0x0100_0013, 0), Some(Key::UpArrow.into()));
        assert_eq!(key_text(0x0100_0030, 0), Some(Key::F1.into()));
        // A key Slint has no key for is dropped.
        assert_eq!(key_text(0x0100_0025, 0), None);
    }
}
