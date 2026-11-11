// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

//! The window the tests drive, shared by the unit tests of the crate and `tests/server.rs`: one
//! element of each kind the Spix calls touch, named as an app's tests would name them, and placed
//! so that nothing overlaps.
//!
//! What happened is shown in one label, `status`, whose text a test reads; the view also answers a
//! `count` for the list, from its model, through the property hook.

use std::rc::Rc;

use gpui_kit::base::{VirtualListScrollHandle, v_virtual_list};
use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::test::TestSupportExt as _;
use gpui_kit::{
    AnyWindowHandle, App, Bounds, Context, Entity, MouseButton, MouseMoveEvent, Pixels, Point,
    SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions, anchored, deferred, div,
    px, size,
};

/// How many rows the list has, which only its model knows: it paints the ones in view.
pub const ROWS: usize = 200;

pub struct TestView {
    pub clicks: u32,
    pub shift_clicks: u32,
    pub double_clicks: u32,
    pub enters: u32,
    pub menu_clicks: u32,
    /// Where a right click opened the menu.
    pub menu: Option<Point<Pixels>>,
    /// The y the pointer reached while the left button was down on the pad.
    pub dragged: Option<f32>,
    pub checked: bool,
    pub input: Option<Entity<InputState>>,
    /// Two inputs with the same id under different parents.
    pub scoped: Vec<(&'static str, Entity<InputState>)>,
    pub scroll: VirtualListScrollHandle,
}

impl TestView {
    fn status(&self) -> SharedString {
        format!(
            "clicks={} shift={} double={} enters={} menu={} dragged={} checked={}",
            self.clicks,
            self.shift_clicks,
            self.double_clicks,
            self.enters,
            self.menu_clicks,
            self.dragged
                .map_or_else(|| "-".to_owned(), |y| format!("{y:.0}")),
            self.checked,
        )
        .into()
    }
}

impl Render for TestView {
    // One element tree, easier to read whole than cut into pieces.
    #[allow(clippy::too_many_lines)]
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.status();
        let input = self.input.clone();
        let mut root = div()
            .id("root")
            .size_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(8.))
            .child(
                div()
                    .id("label")
                    .test_support()
                    .aria_label("Hello")
                    .h(px(24.))
                    .child("Hello"),
            )
            .child(
                div()
                    .id("status")
                    .test_support()
                    .aria_label(status.clone())
                    .h(px(24.))
                    .child(status),
            )
            .child(
                div()
                    .id("toolbar")
                    .test_support()
                    .flex()
                    .gap(px(8.))
                    .h(px(36.))
                    .child(Button::new("button").label("Press").on_click(cx.listener(
                        |this, event: &gpui_kit::ClickEvent, _, cx| {
                            this.clicks += 1;
                            if event.modifiers().shift {
                                this.shift_clicks += 1;
                            }
                            if event.click_count() == 2 {
                                this.double_clicks += 1;
                            }
                            cx.notify();
                        },
                    )))
                    .child(Button::new("off").label("Off").disabled(true))
                    .child(
                        Checkbox::new("check")
                            .label("Check")
                            .checked(self.checked)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.checked = *checked;
                                cx.notify();
                            })),
                    ),
            );
        if let Some(input) = input {
            root = root.child(Input::new(&input).id("field").w(px(300.)).h(px(32.)));
        }
        for (scope, state) in &self.scoped {
            root = root.child(
                div()
                    .id(*scope)
                    .test_support()
                    .child(Input::new(state).id("composerField").w(px(300.)).h(px(32.))),
            );
        }
        root.child(
            // A press, a drag and a release: the pad says how far the pointer got.
            div()
                .id("pad")
                .test_support()
                .w(px(300.))
                .h(px(120.))
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        this.dragged = Some(f32::from(event.position.y));
                        cx.notify();
                    }
                })),
        )
        .child(
            div()
                .id("anchor")
                .test_support()
                .w(px(300.))
                .h(px(40.))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, event: &gpui_kit::MouseDownEvent, _, cx| {
                        this.menu = Some(event.position);
                        cx.notify();
                    }),
                ),
        )
        .child(
            div()
                .id("list")
                .test_support()
                .w(px(300.))
                .h(px(200.))
                .child(
                    v_virtual_list(
                        cx.entity(),
                        "rows",
                        Rc::new(vec![size(px(300.), px(24.)); ROWS]),
                        |_, range, _, _| {
                            range
                                .map(|index| {
                                    let text: SharedString = format!("Row {index}").into();
                                    div()
                                        .id(("row", index))
                                        .test_support()
                                        .aria_label(text.clone())
                                        .h(px(24.))
                                        .child(text)
                                })
                                .collect()
                        },
                    )
                    .track_scroll(&self.scroll),
                ),
        )
        .children(self.menu.map(|at| {
            // A popup: painted under a root of its own, over everything.
            deferred(
                anchored().position(at).child(
                    div()
                        .id("menu")
                        .test_support()
                        .w(px(140.))
                        .h(px(40.))
                        .child(
                            div()
                                .id("menu-item")
                                .test_support()
                                .aria_label("Menu item")
                                .w(px(140.))
                                .h(px(40.))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.menu_clicks += 1;
                                    this.menu = None;
                                    cx.notify();
                                }))
                                .child("Menu item"),
                        ),
                ),
            )
        }))
    }
}

/// Opens the window, with its input registered the way an app would register it, and the property
/// hook for the list's `count`.
pub fn open(cx: &mut App) -> Result<(AnyWindowHandle, Entity<TestView>), String> {
    gpui_kit::init(cx);
    let bounds = Bounds::centered(None, size(px(400.), px(700.)), cx);
    let (handle, view) = gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("spix_gpui test".into()),
                ..TitlebarOptions::default()
            }),
            ..WindowOptions::default()
        },
        cx,
        |window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx));
            let room = cx.new(|cx| InputState::new(window, cx));
            let thread = cx.new(|cx| InputState::new(window, cx));
            spix_gpui::register_input("roomComposer/composerField", room.clone());
            spix_gpui::register_input("threadPane/composerField", thread.clone());
            let view = cx.new(|cx| {
                cx.subscribe(&input, |this: &mut TestView, _, event: &InputEvent, _| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.enters += 1;
                    }
                })
                .detach();
                TestView {
                    clicks: 0,
                    shift_clicks: 0,
                    double_clicks: 0,
                    enters: 0,
                    menu_clicks: 0,
                    menu: None,
                    dragged: None,
                    checked: false,
                    input: Some(input.clone()),
                    scoped: vec![("roomComposer", room), ("threadPane", thread)],
                    scroll: VirtualListScrollHandle::new(),
                }
            });
            spix_gpui::register_input("field", input);
            view
        },
    )
    .map_err(|error| error.to_string())?;
    spix_gpui::on_property(
        |_, path, name| match (path.last().map(String::as_str), name) {
            (Some("list"), "count") => Some(ROWS.to_string()),
            _ => None,
        },
    );
    Ok((handle, view))
}
