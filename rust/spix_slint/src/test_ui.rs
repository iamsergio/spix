// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The window the unit tests drive: one element of each kind the Spix calls touch, named as the
//! GUI test contract names the elements of an app, and placed so that nothing overlaps.
//!
//! The testing backend is set on the calling thread once: it can only be set once per thread, and
//! libtest hands a thread to one test after another. Mock time, no event loop: the tests read the
//! window and dispatch keys to it, and nothing waits for one. Opening the menu and focusing the
//! field are calls into the component, because a click is Slint's own `single_click`, which needs
//! the event loop of `tests/server.rs`.

use i_slint_backend_testing::TestingBackend;
use i_slint_backend_testing::testing_backend::TestingBackendOptions;
use slint::platform::set_platform;

slint::slint! {
    export component TestWindow inherits Window {
        // What the field says when a key accepts it (Return, Enter).
        in-out property <string> accepted;
        // What the field holds, through it.
        in-out property <string> field-text <=> field.text;
        in property <int> row-count: 3;

        width: 400px;
        height: 300px;

        public function open-menu() {
            menu.show();
        }

        public function focus-field() {
            field.focus();
        }

        Text {
            accessible-role: text;
            accessible-id: "label";
            text: "Hello";
        }

        field := TextInput {
            accessible-id: "field";
            x: 0;
            y: 60px;
            width: 200px;
            height: 30px;

            accepted => {
                root.accepted = "accepted";
            }
        }

        Rectangle {
            accessible-role: button;
            accessible-id: "button";
            accessible-label: "Press";
            accessible-enabled: true;
            accessible-checkable: true;
            accessible-checked: false;
            x: 0;
            y: 110px;
            width: 120px;
            height: 40px;
        }

        Rectangle {
            accessible-role: list;
            accessible-id: "list";
            accessible-item-count: root.row-count;
            x: 150px;
            y: 110px;
            width: 120px;
            height: 90px;

            for index in root.row-count: Rectangle {
                accessible-role: list-item;
                accessible-id: "row_" + index;
                y: index * 30px;
                width: parent.width;
                height: 30px;
            }
        }

        // A path that nests for real: the search walks the descendants of `outer` to find `inner`.
        Rectangle {
            accessible-role: button;
            accessible-id: "outer";
            x: 280px;
            y: 10px;
            width: 100px;
            height: 100px;

            Rectangle {
                accessible-role: button;
                accessible-id: "inner";
                x: 30px;
                y: 30px;
                width: 40px;
                height: 40px;
            }
        }

        // Not in the search: Slint skips what isn't visible, children of it included.
        Rectangle {
            accessible-role: button;
            accessible-id: "hidden";
            visible: false;
            x: 0;
            y: 210px;
            width: 100px;
            height: 40px;

            Rectangle {
                accessible-role: button;
                accessible-id: "hidden-child";
            }
        }

        // A menu over the popup's own elements: what the GUI tests do with their menus, which hang
        // off the window while they are open, and whose items are addressed from it.
        anchor := Rectangle {
            accessible-role: button;
            accessible-id: "anchor";
            x: 0;
            y: 260px;
            width: 80px;
            height: 40px;

            menu := PopupWindow {
                x: 100px;
                y: 50px;
                width: 120px;
                height: 60px;

                Rectangle {
                    accessible-role: button;
                    accessible-id: "popup-item";
                    x: 10px;
                    y: 10px;
                    width: 100px;
                    height: 40px;
                }
            }
        }
    }
}

/// The window of the tests, with the testing backend set on this thread's first use of it.
pub(crate) fn window() -> TestWindow {
    let _ = set_platform(Box::new(TestingBackend::new(TestingBackendOptions {
        mock_time: true,
        threading: false,
        ..Default::default()
    })));
    TestWindow::new().expect("the testing backend of this thread to be set")
}
