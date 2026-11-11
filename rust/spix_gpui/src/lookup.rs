// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! Finding an element in the registry GPUI Kit fills while a frame paints, and the items a path
//! finds.
//!
//! The registry knows each element by its full path: the chain of GPUI element ids from the root
//! of the window down to it, in which only ancestors that have an id appear and components put
//! type-name segments of their own. A Spix path can't spell that, so it is matched as a
//! subsequence of the named segments.

use gpui_kit::base::test_support::{self, ElementSnapshot};
use gpui_kit::{ElementId, Window};

/// What a path found: the window itself, or an element, kept as its full path so that it is read
/// afresh from the registry each time (it moves when a list scrolls).
#[derive(Clone, Debug)]
pub(crate) enum Item {
    Window,
    Element(Vec<ElementId>),
}

/// The name a Spix path spells for a segment of a registry path, if the segment is one a person
/// gave an id: `.id("name")` and `.id(("name", 3))`, the latter as `name-3`.
fn segment(id: &ElementId) -> Option<String> {
    match id {
        ElementId::Name(name) => Some(name.to_string()),
        ElementId::NamedInteger(name, index) => Some(format!("{name}-{index}")),
        _ => None,
    }
}

/// The named segments of a registry path from the root down, spelled as a Spix path spells them,
/// the element's own id last. This is what the app's hooks are told, so that they can tell two
/// elements with the same id in different scopes apart.
pub(crate) fn spelled(path: &[ElementId]) -> Vec<String> {
    let mut names: Vec<String> = path.iter().filter_map(segment).collect();
    // An element whose own id is not a name still has one to be told by.
    if path.last().is_some_and(|id| segment(id).is_none()) {
        names.extend(path.last().map(ToString::to_string));
    }
    names
}

/// Whether the registry path `path` is what `names` spells: the last name is the element's own
/// id, and the names before it are among the named segments of its ancestors, in that order, not
/// necessarily next to each other.
pub(crate) fn matches(path: &[ElementId], names: &[&str]) -> bool {
    let Some((own, ancestors)) = names.split_last() else {
        return false;
    };
    let Some((last, above)) = path.split_last() else {
        return false;
    };
    if segment(last).as_deref() != Some(own) {
        return false;
    }
    let mut wanted = ancestors.iter();
    let mut next = wanted.next();
    for id in above {
        if let (Some(name), Some(spelled)) = (next, segment(id))
            && **name == spelled
        {
            next = wanted.next();
        }
    }
    next.is_none()
}

/// The elements the last drawn frame registered that `names` spells, best first: the visible ones
/// before the others, then top to bottom and left to right by the corner of their bounds (the
/// order of GPUI Kit's own `find_all`), then the outer one before the one inside it. The registry
/// itself is unordered, so this order is what makes "the first one" mean something.
pub(crate) fn find(window: &Window, names: &[&str]) -> Option<Item> {
    let mut found: Vec<ElementSnapshot> = test_support::snapshots(window)
        .into_iter()
        .filter(|snapshot| matches(snapshot.path(), names))
        .collect();
    found.sort_by_key(|snapshot| {
        let at = snapshot.bounds().origin;
        (!snapshot.visible(), at.y, at.x, snapshot.path().len())
    });
    found
        .first()
        .map(|snapshot| Item::Element(snapshot.path().to_vec()))
}

/// The registry's current facts about the element at `path`, with the semantics of the native
/// editor behind it where a component has one: an input's value. `None` once it is gone.
pub(crate) fn resolve(window: &Window, path: &[ElementId]) -> Option<ElementSnapshot> {
    let (id, scope) = path.split_last()?;
    test_support::find_all(window, scope, id)
        .into_iter()
        .find(|snapshot| snapshot.path() == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &'static str) -> ElementId {
        ElementId::Name(text.into())
    }

    fn path() -> Vec<ElementId> {
        vec![
            name("root"),
            ElementId::Integer(7),
            name("composer"),
            name("gpui_base::Button"),
            name("send"),
        ]
    }

    #[test]
    fn the_last_name_is_the_elements_own_id() {
        assert!(matches(&path(), &["send"]));
        assert!(!matches(&path(), &["composer"]));
        assert!(!matches(&path(), &[]));
    }

    #[test]
    fn the_names_before_it_are_a_subsequence_of_the_ancestors() {
        assert!(matches(&path(), &["composer", "send"]));
        assert!(matches(&path(), &["root", "send"]));
        assert!(matches(&path(), &["root", "composer", "send"]));
        assert!(!matches(&path(), &["send", "composer", "send"]));
        assert!(!matches(&path(), &["composer", "root", "send"]));
        assert!(!matches(&path(), &["other", "send"]));
    }

    #[test]
    fn a_named_integer_is_spelled_with_a_dash() {
        let row = vec![name("list"), ElementId::NamedInteger("row".into(), 3)];
        assert!(matches(&row, &["list", "row-3"]));
        assert!(!matches(&row, &["row-4"]));
    }
}
