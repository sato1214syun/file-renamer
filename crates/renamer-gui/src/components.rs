//! 小さな共通 UI 部品。クリック処理は呼び出し側で `.on_click(...)` を付ける。

use gpui::{Div, ElementId, SharedString, Stateful, div, prelude::*, px, rgb};

use crate::theme::Theme;

pub fn button(
    t: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
) -> Stateful<Div> {
    let base = div()
        .id(id)
        .flex_none()
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(t.border))
        .whitespace_nowrap()
        .child(label.into());
    if enabled {
        let hover = t.hover;
        base.bg(rgb(t.panel))
            .text_color(rgb(t.text))
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(hover)))
    } else {
        base.bg(rgb(t.bg)).text_color(rgb(t.disabled))
    }
}

pub fn primary_button(
    t: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
) -> Stateful<Div> {
    let base = div()
        .id(id)
        .flex_none()
        .px_4()
        .py_1()
        .rounded_md()
        .whitespace_nowrap()
        .child(label.into());
    if enabled {
        let hover = t.accent_hover;
        base.bg(rgb(t.accent))
            .text_color(rgb(t.on_accent))
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(hover)))
    } else {
        base.bg(rgb(t.border)).text_color(rgb(t.disabled))
    }
}

/// チェックボックス＋ラベル（クリック全体で切り替え）。
pub fn check_label(
    t: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> Stateful<Div> {
    let hover = t.hover;
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .whitespace_nowrap()
        .hover(move |s| s.bg(rgb(hover)))
        .child(checkbox(t, on))
        .child(label.into())
}

/// 表示専用のチェックボックス。
pub fn checkbox(t: &Theme, checked: bool) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(14.0))
        .rounded_sm()
        .border_1()
        .text_xs()
        .border_color(rgb(if checked { t.accent } else { t.disabled }))
        .bg(rgb(if checked { t.accent } else { t.input_bg }))
        .text_color(rgb(t.on_accent))
        .when(checked, |d| d.child("✓"))
}

/// 小さな記号ボタン（↑ ↓ ✕ など）。
pub fn icon_button(
    t: &Theme,
    id: impl Into<ElementId>,
    icon: &'static str,
    enabled: bool,
) -> Stateful<Div> {
    let hover = t.hover;
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(22.0))
        .rounded_sm()
        .text_color(rgb(if enabled { t.text } else { t.disabled }))
        .when(enabled, move |d| {
            d.cursor_pointer().hover(move |s| s.bg(rgb(hover)))
        })
        .child(icon)
}
