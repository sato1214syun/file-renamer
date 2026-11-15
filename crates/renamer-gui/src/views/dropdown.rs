//! ドロップダウンリスト。

use std::rc::Rc;

use gpui::{
    ClickEvent, Context, Div, MouseDownEvent, SharedString, anchored, deferred, div, prelude::*,
    px, rgb,
};

use crate::app::{DropdownKey, RenamerView};

pub(crate) const DROPDOWN_HEIGHT: f32 = 28.0;

/// 項目が選ばれたときの処理
pub(crate) type OnSelect = Rc<dyn Fn(&mut RenamerView, usize, &mut Context<RenamerView>)>;

impl RenamerView {
    /// ドロップダウン。返り値の幅は呼び出し側で `.w(...)` や `.flex_1()` で指定する。
    pub(crate) fn dropdown(
        &self,
        key: DropdownKey,
        options: Vec<SharedString>,
        selected: usize,
        on_select: OnSelect,
        cx: &mut Context<Self>,
    ) -> Div {
        let t = self.theme();
        let open = self.open_dropdown == Some(key);
        let label = options.get(selected).cloned().unwrap_or_default();
        let id = key.index();
        let hover = t.hover;

        let button = div()
            .id(("dropdown", id))
            .w_full()
            .h(px(DROPDOWN_HEIGHT))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(if open { t.accent } else { t.border }))
            .bg(rgb(t.input_bg))
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(hover)))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(label),
            )
            .child(
                div()
                    .flex_none()
                    .text_xs()
                    .text_color(rgb(t.muted))
                    .child(if open { "▲" } else { "▼" }),
            )
            .on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_dropdown(key, cx)),
            );

        let mut root = div().relative().child(button);
        if open {
            let items: Vec<_> = options
                .into_iter()
                .enumerate()
                .map(|(i, option)| {
                    let on_select = on_select.clone();
                    let is_selected = i == selected;
                    div()
                        .id(("dropdown-item", id * 100 + i))
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py_1()
                        .whitespace_nowrap()
                        .cursor_pointer()
                        .bg(rgb(if is_selected { t.accent_soft } else { t.panel }))
                        .hover(move |s| s.bg(rgb(hover)))
                        .child(
                            div()
                                .w(px(16.0))
                                .flex_none()
                                .text_color(rgb(t.accent))
                                .child(if is_selected { "✓" } else { "" }),
                        )
                        .child(option)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.open_dropdown = None;
                            on_select(this, i, cx);
                            cx.notify();
                        }))
                })
                .collect();

            let menu =
                div()
                    .id(("dropdown-menu", id))
                    .occlude()
                    .min_w(px(180.0))
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(t.border))
                    .bg(rgb(t.panel))
                    .text_color(rgb(t.text))
                    .text_sm()
                    .shadow_lg()
                    .children(items)
                    .on_mouse_down_out(cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        this.close_dropdown(key, cx)
                    }));

            // ボタンの真下に、ほかの要素より手前に表示する
            root = root.child(
                div()
                    .absolute()
                    .top(px(DROPDOWN_HEIGHT + 2.0))
                    .left_0()
                    .child(deferred(anchored().snap_to_window().child(menu)).with_priority(1)),
            );
        }
        root
    }
}

/// `&'static str` の一覧を表示用に変換する。
pub(crate) fn labels(options: &[&'static str]) -> Vec<SharedString> {
    options.iter().map(|s| SharedString::from(*s)).collect()
}
