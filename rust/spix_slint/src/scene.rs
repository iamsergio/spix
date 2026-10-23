// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! What a Spix path finds in a window's Slint scene, and what Spix says about it: the Rust side of
//! what `libs/Scenes/Slint/` does for a C++ Slint app. Its findings in PROGRESS.md apply, with one
//! difference: `visit_descendants` walks real ancestry, so nesting doesn't have to be guessed from
//! geometry. The search skips elements that aren't visible, and never visits the window's own
//! element, so a path that stops at the window has no element behind it.

use std::ops::ControlFlow;

use i_slint_backend_testing::ElementHandle;
use slint::{LogicalPosition, LogicalSize, Window};

/// What a path found: an element, or the window itself.
#[derive(Clone)]
pub(crate) enum Item {
    Element(ElementHandle),
    Window,
}

/// A rectangle in logical pixels, as `getBoundingBox` reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Finds the item under `names`, each one an `accessible-id` among the descendants of the previous
/// match, the first one in tree order winning. With no `names` the item is the window itself.
///
/// Returns `None` when a name is missing: Spix then reports the path as "item not found".
pub(crate) fn find_item(root: &ElementHandle, names: &[&str]) -> Option<Item> {
    let mut parent = root.clone();
    let mut found = Item::Window;
    for name in names {
        let element = parent.visit_descendants(|element| {
            if element.accessible_id().as_deref() == Some(*name) {
                ControlFlow::Break(element)
            } else {
                ControlFlow::Continue(())
            }
        })?;
        parent = element.clone();
        found = Item::Element(element);
    }
    Some(found)
}

/// The bounds of `item` in screen coordinates: where the window sits on the screen plus where the
/// item sits in the window, which is what Qt's `mapToGlobal` gives the Qt frontend.
pub(crate) fn bounds(window: &Window, item: &Item) -> Rect {
    let scale = f64::from(window.scale_factor());
    let screen = window.position();
    let (at, size) = geometry(window, item);
    Rect {
        x: f64::from(screen.x) / scale + f64::from(at.x),
        y: f64::from(screen.y) / scale + f64::from(at.y),
        width: f64::from(size.width),
        height: f64::from(size.height),
    }
}

/// Where the item is in the window and how big it is, all in logical pixels. `window.size()` is
/// physical, and the window's own position on screen is none of this.
fn geometry(window: &Window, item: &Item) -> (LogicalPosition, LogicalSize) {
    match item {
        Item::Element(element) => (element.absolute_position(), element.size()),
        Item::Window => (
            LogicalPosition::new(0.0, 0.0),
            window.size().to_logical(window.scale_factor()),
        ),
    }
}

/// Whether Spix sees the item as visible. Elements are only found while they are visible, so only
/// the window itself can answer false.
pub(crate) fn visible(window: &Window, item: &Item) -> bool {
    match item {
        Item::Element(_) => true,
        Item::Window => window.is_visible(),
    }
}

/// What Spix reads from the property `name`, or `None` when the item has no such property: Spix
/// then reports the empty string, as C++ Spix does.
pub(crate) fn string_property(item: &Item, name: &str) -> Option<String> {
    let Item::Element(element) = item else {
        // The window itself has no accessibility properties.
        return None;
    };
    match name {
        // Text inputs expose their text as accessible value; text and buttons as their label.
        "text" => element
            .accessible_value()
            .or_else(|| element.accessible_label())
            .map(|text| text.to_string()),
        "checked" => element
            .accessible_checked()
            .map(|checked| checked.to_string()),
        "enabled" => element
            .accessible_enabled()
            .map(|enabled| enabled.to_string()),
        "count" => element
            .accessible_item_count()
            .map(|count| count.to_string()),
        _ => None,
    }
}

/// Puts `value` into the property `name`. False when the item can't take it, which Spix reports.
pub(crate) fn set_string_property(item: &Item, name: &str, value: &str) -> bool {
    let Item::Element(element) = item else {
        return false;
    };
    match name {
        // The accessible action of a text input sets its text and tells it that it was edited.
        "text" => {
            element.set_accessible_value(value);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use i_slint_backend_testing::ElementRoot;
    use slint::ComponentHandle;

    use super::*;
    use crate::test_ui;

    // INFRA-004
    #[test]
    fn finds_the_window_itself_and_every_element_named_on_the_way() {
        let window = test_ui::window();
        let root = window.root_element();

        // The window: the search never visits the window's own element, so it is what an empty
        // path below the window name resolves to.
        assert!(matches!(find_item(&root, &[]), Some(Item::Window)));
        assert!(matches!(
            find_item(&root, &["label"]),
            Some(Item::Element(_))
        ));

        // Nesting is real ancestry: `inner` is a descendant of `outer`, and of the window, but
        // nothing else of its name lies under `outer`.
        assert!(matches!(
            find_item(&root, &["outer", "inner"]),
            Some(Item::Element(_))
        ));
        assert!(find_item(&root, &["outer", "label"]).is_none());

        // The rows of the `for` are named after their index.
        assert!(matches!(
            find_item(&root, &["list", "row_1"]),
            Some(Item::Element(_))
        ));
        assert!(find_item(&root, &["list", "row_3"]).is_none());

        // Hidden elements, and their children, are not in the search at all.
        assert!(find_item(&root, &["hidden"]).is_none());
        assert!(find_item(&root, &["hidden-child"]).is_none());

        assert!(find_item(&root, &["nowhere"]).is_none());
    }

    // INFRA-004
    #[test]
    fn reads_the_properties_the_tests_read() {
        let window = test_ui::window();
        let root = window.root_element();
        let item = |name: &str| find_item(&root, &[name]).unwrap();

        // A text element says what it shows; a text input what it holds (its accessible value).
        assert_eq!(
            string_property(&item("label"), "text"),
            Some("Hello".to_owned())
        );
        assert_eq!(string_property(&item("field"), "text"), Some(String::new()));

        assert_eq!(
            string_property(&item("button"), "checked"),
            Some("false".to_owned())
        );
        assert_eq!(
            string_property(&item("button"), "enabled"),
            Some("true".to_owned())
        );
        assert_eq!(
            string_property(&item("list"), "count"),
            Some("3".to_owned())
        );

        // The window itself has no accessibility properties, and a property nothing sets isn't
        // there: Spix then reports the empty string.
        assert_eq!(string_property(&Item::Window, "text"), None);
        assert_eq!(string_property(&item("label"), "bogus"), None);
    }

    // INFRA-004
    #[test]
    fn sets_the_text_of_a_field_and_the_checked_of_a_checkable_element() {
        let window = test_ui::window();
        let root = window.root_element();
        let field = find_item(&root, &["field"]).unwrap();
        assert!(set_string_property(&field, "text", "someone"));
        assert_eq!(string_property(&field, "text"), Some("someone".to_owned()));

        let button = find_item(&root, &["button"]).unwrap();
        // Reading `checked` is what the tests do with it; setting it goes through a click.
        assert!(!set_string_property(&button, "checked", "true"));
        assert!(!set_string_property(&Item::Window, "text", "nope"));
    }

    // INFRA-004
    #[test]
    fn a_menu_item_in_a_popup_is_found_where_the_tests_address_it() {
        let ui = test_ui::window();
        let root = ui.root_element();

        // The menu opens as it does in the app when a test clicks its anchor, here from the
        // component itself: a click is Slint's own `single_click`, which needs an event loop
        // (see `tests/server.rs`).
        ui.invoke_open_menu();

        // While it is open, its items stand in the search next to the elements of the main tree,
        // so a test addresses a menu item from the window, and they hang off
        // the window, not off the anchor below the popup.
        assert!(matches!(
            find_item(&root, &["popup-item"]),
            Some(Item::Element(_))
        ));
        assert!(find_item(&root, &["anchor", "popup-item"]).is_none());
    }

    // INFRA-004
    #[test]
    fn the_bounds_are_where_the_item_sits_on_the_screen() {
        let window = test_ui::window();
        let root = window.root_element();
        let inner = find_item(&root, &["outer", "inner"]).unwrap();
        let inner_bounds = bounds(window.window(), &inner);

        // The window itself is where it is on the screen, and big enough to hold the element: the
        // testing backend opens its own window around the .slint one.
        let whole = bounds(window.window(), &Item::Window);
        assert!(whole.x.abs() < 0.01, "{whole:?}");
        assert!(whole.y.abs() < 0.01, "{whole:?}");
        assert!(
            whole.width >= inner_bounds.x + inner_bounds.width,
            "{inner_bounds:?} in {whole:?}"
        );
        assert!(
            whole.height >= inner_bounds.y + inner_bounds.height,
            "{whole:?}"
        );

        // The element's own geometry: `inner` sits 30px into `outer`, which sits at x = 280.
        assert!((inner_bounds.x - 310.0).abs() < 1.0, "{inner_bounds:?}");
        assert!((inner_bounds.y - 40.0).abs() < 1.0, "{inner_bounds:?}");
        assert!((inner_bounds.width - 40.0).abs() < 1.0, "{inner_bounds:?}");
    }
}
