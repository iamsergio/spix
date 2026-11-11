// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The input the server sends a window: GPUI's own platform events, through `dispatch_event`, so
//! that hit testing, focus, hover and drag run as they do for a user. A frame is drawn after each
//! event, which both settles what the event changed and refreshes the registry the next lookup
//! reads.
//!
//! The held modifiers are the window's own (`Window::modifiers`, which a modifiers-changed event
//! updates), and every pointer event carries them.

use std::cell::Cell;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    App, InputEvent, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, ModifiersChangedEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta,
    ScrollEvent, Window, point, px,
};
use spix_server::{Button, Modifier, PointerEvent};

/// Logical pixels one notch of a mouse wheel scrolls: the 60 `spix_slint` uses for Slint, so that
/// a test scrolls a list as far in either toolkit.
const PIXELS_PER_NOTCH: f32 = 60.0;
/// Qt's `angleDelta` of one notch: eighths of a degree, 15 degrees.
const ANGLE_PER_NOTCH: f32 = 120.0;

thread_local! {
    /// The button that is down, which a move while dragging must say it holds. There is one
    /// pointer, so one button, whichever window it is over.
    static HELD: Cell<Option<MouseButton>> = const { Cell::new(None) };
}

fn gpui_button(button: Button) -> MouseButton {
    match button {
        Button::Left => MouseButton::Left,
        Button::Right => MouseButton::Right,
        Button::Middle => MouseButton::Middle,
    }
}

#[allow(clippy::cast_possible_truncation)] // Pixels are small.
fn position(x: f64, y: f64) -> Point<Pixels> {
    point(px(x as f32), px(y as f32))
}

/// Sends one pointer event at (`x`, `y`) to the window. A wheel turn is in Qt's `angleDelta`
/// units, a positive y towards the start of the content; GPUI's pixel delta has the same sign
/// (a negative y moves a list to its end), so only the unit changes.
#[allow(clippy::cast_precision_loss)] // Wheel deltas are small.
pub(crate) fn pointer(window: &mut Window, x: f64, y: f64, event: PointerEvent, cx: &mut App) {
    let at = position(x, y);
    match event {
        PointerEvent::Move => move_to(window, at, cx),
        PointerEvent::Press(button) => press(window, at, button, 1, cx),
        PointerEvent::Release(button) => release(window, at, button, 1, cx),
        PointerEvent::Wheel { angle_x, angle_y } => {
            // The pointer is over the content first, as a user's is, for the hover that picks
            // the scrolled element.
            move_to(window, at, cx);
            let to_pixels = |angle: i32| px(angle as f32 / ANGLE_PER_NOTCH * PIXELS_PER_NOTCH);
            window.dispatch_event(
                ScrollEvent {
                    position: at,
                    delta: ScrollDelta::Pixels(point(to_pixels(angle_x), to_pixels(angle_y))),
                    modifiers: window.modifiers(),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
        }
    }
}

fn move_to(window: &mut Window, at: Point<Pixels>, cx: &mut App) {
    window.dispatch_event(
        MouseMoveEvent {
            position: at,
            pressed_button: HELD.get(),
            modifiers: window.modifiers(),
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

fn press(window: &mut Window, at: Point<Pixels>, button: Button, count: usize, cx: &mut App) {
    let button = gpui_button(button);
    HELD.set(Some(button));
    window.dispatch_event(
        MouseDownEvent {
            button,
            position: at,
            modifiers: window.modifiers(),
            click_count: count,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

fn release(window: &mut Window, at: Point<Pixels>, button: Button, count: usize, cx: &mut App) {
    let button = gpui_button(button);
    HELD.set(None);
    window.dispatch_event(
        MouseUpEvent {
            button,
            position: at,
            modifiers: window.modifiers(),
            click_count: count,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

/// A click, or the two of a double click, at (`x`, `y`). GPUI counts a double click in the event
/// (`click_count`), where the platforms count it for it, so the second press says 2.
pub(crate) fn click(
    window: &mut Window,
    x: f64,
    y: f64,
    button: Button,
    times: usize,
    cx: &mut App,
) {
    let at = position(x, y);
    move_to(window, at, cx);
    for count in 1..=times {
        press(window, at, button, count, cx);
        release(window, at, button, count, cx);
    }
}

/// Presses or lets go of a modifier key.
pub(crate) fn modifier(window: &mut Window, modifier: Modifier, pressed: bool, cx: &mut App) {
    let mut modifiers = window.modifiers();
    match modifier {
        Modifier::Shift => modifiers.shift = pressed,
        Modifier::Control => modifiers.control = pressed,
        Modifier::Alt => modifiers.alt = pressed,
        Modifier::Meta => modifiers.platform = pressed,
    }
    window.dispatch_event(
        ModifiersChangedEvent {
            modifiers,
            capslock: window.capslock(),
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

/// Types `text` as a keyboard would: one keystroke with its character per code point, to whatever
/// has the focus.
pub(crate) fn input_text(window: &mut Window, text: &str, cx: &mut App) {
    for character in text.chars() {
        let typed = character.to_string();
        let key = if character == ' ' {
            "space".to_owned()
        } else {
            character.to_lowercase().collect()
        };
        window.dispatch_keystroke(
            Keystroke {
                modifiers: Modifiers::default(),
                key,
                key_char: Some(typed),
            },
            cx,
        );
        window.render_frame(cx);
    }
}

/// Presses and releases the key with the Spix key code `key_code` (Qt's `Qt::Key`) under
/// `modifiers` and the ones the window already holds. False when GPUI has no key for the code,
/// which Spix then drops silently.
pub(crate) fn enter_key(
    window: &mut Window,
    key_code: i32,
    modifiers: &[Modifier],
    cx: &mut App,
) -> bool {
    let mut held = window.modifiers();
    for modifier in modifiers {
        match modifier {
            Modifier::Shift => held.shift = true,
            Modifier::Control => held.control = true,
            Modifier::Alt => held.alt = true,
            Modifier::Meta => held.platform = true,
        }
    }
    let Some(keystroke) = keystroke(key_code, held) else {
        return false;
    };
    // Enter and Tab are bindings only: GPUI's simulated text input would put a newline or a tab
    // in a field after the binding ran. Every other key also types its character, as the
    // platform's input method does for a keyboard.
    if matches!(keystroke.key.as_str(), "enter" | "tab") {
        window.dispatch_event(
            KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            }
            .to_platform_input(),
            cx,
        );
    } else {
        window.dispatch_keystroke(keystroke.clone().with_simulated_ime(), cx);
    }
    // A button acts on key-up, so the pair is always completed.
    window.dispatch_event(KeyUpEvent { keystroke }.to_platform_input(), cx);
    window.render_frame(cx);
    true
}

/// The keystroke for a Spix key code under `modifiers`, or `None` for a code with no key in GPUI.
fn keystroke(key_code: i32, mut modifiers: Modifiers) -> Option<Keystroke> {
    let named = match key_code {
        0x0100_0000 => Some("escape"),
        0x0100_0001 => Some("tab"),
        0x0100_0002 => {
            modifiers.shift = true;
            Some("tab")
        }
        0x0100_0003 => Some("backspace"),
        0x0100_0004 | 0x0100_0005 => Some("enter"),
        0x0100_0006 => Some("insert"),
        0x0100_0007 => Some("delete"),
        0x0100_0010 => Some("home"),
        0x0100_0011 => Some("end"),
        0x0100_0012 => Some("left"),
        0x0100_0013 => Some("up"),
        0x0100_0014 => Some("right"),
        0x0100_0015 => Some("down"),
        0x0100_0016 => Some("pageup"),
        0x0100_0017 => Some("pagedown"),
        _ => None,
    };
    if let Some(key) = named {
        return Some(Keystroke {
            modifiers,
            key: key.to_owned(),
            key_char: None,
        });
    }

    // F1 to F24 are consecutive from Qt's 0x01000030.
    if (0x0100_0030..=0x0100_0047).contains(&key_code) {
        return Some(Keystroke {
            modifiers,
            key: format!("f{}", key_code - 0x0100_0030 + 1),
            key_char: None,
        });
    }

    // Printable ASCII: Qt's codes for letters are the upper case ones. The key is the one on the
    // keyboard, lower case, and the character is what it types, which Shift changes; with
    // control, alt or super held it types nothing and is a shortcut.
    if (0x20..=0x7e).contains(&key_code) {
        let character = char::from_u32(u32::try_from(key_code).ok()?)?;
        let lower = character.to_ascii_lowercase();
        let inserts = !(modifiers.control || modifiers.alt || modifiers.platform);
        let typed = if modifiers.shift {
            character.to_ascii_uppercase()
        } else {
            lower
        };
        return Some(Keystroke {
            modifiers,
            key: if lower == ' ' {
                "space".to_owned()
            } else {
                lower.to_string()
            },
            key_char: inserts.then(|| typed.to_string()),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spix_key_codes_become_gpui_keys() {
        let plain = Modifiers::default();
        let enter = keystroke(0x0100_0004, plain);
        assert_eq!(enter.map(|key| key.key), Some("enter".to_owned()));
        let f1 = keystroke(0x0100_0030, plain);
        assert_eq!(f1.map(|key| key.key), Some("f1".to_owned()));
        let down = keystroke(0x0100_0015, plain);
        assert_eq!(down.map(|key| key.key), Some("down".to_owned()));
        // A key GPUI has no name for here is dropped.
        assert!(keystroke(0x0100_0025, plain).is_none());
    }

    #[test]
    fn letters_type_lower_case_unless_shift_or_a_shortcut_modifier_is_held() {
        let key = keystroke(0x56, Modifiers::default());
        assert_eq!(key.and_then(|key| key.key_char), Some("v".to_owned()));
        let shifted = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        let key = keystroke(0x56, shifted);
        assert_eq!(key.and_then(|key| key.key_char), Some("V".to_owned()));
        let control = Modifiers {
            control: true,
            ..Modifiers::default()
        };
        let key = keystroke(0x41, control);
        assert_eq!(
            key.map(|key| (key.key, key.key_char)),
            Some(("a".to_owned(), None))
        );
        let space = keystroke(0x20, Modifiers::default());
        assert_eq!(space.map(|key| key.key), Some("space".to_owned()));
    }

    #[test]
    fn backtab_is_shift_tab() {
        let key = keystroke(0x0100_0002, Modifiers::default());
        assert_eq!(
            key.map(|key| (key.key, key.modifiers.shift)),
            Some(("tab".to_owned(), true))
        );
    }
}
