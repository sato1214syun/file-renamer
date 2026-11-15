use gpui::{ClickEvent, Context, div, prelude::*, px, rgb};

use crate::app::RenamerView;
use crate::components::{button, primary_button};

impl RenamerView {
    pub(crate) fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme();
        let s = self.plan.summary();
        let can_execute = self.plan.can_execute();

        // ルールのエラーは通常のメッセージより優先して表示する
        let (message, is_error) = match &self.plan.rule_error {
            Some(err) => (format!("ルール {}: {}", err.index + 1, err.message), true),
            None => (self.status.clone(), self.status_is_error),
        };

        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .bg(rgb(t.panel))
            .border_t_1()
            .border_color(rgb(t.border))
            .child(div().flex_none().whitespace_nowrap().child(format!(
                "全 {} 件 ／ 変更 {} ／ 変更なし {} ／ 除外 {} ／ ",
                s.total, s.ready, s.unchanged, s.excluded
            )))
            .child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_color(rgb(if s.errors > 0 { t.error } else { t.text }))
                    .child(format!("エラー {}", s.errors)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_color(rgb(if is_error { t.error } else { t.muted }))
                    .child(message),
            )
            .child(
                button(t, "undo", "元に戻す", self.can_undo).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        if this.can_undo {
                            this.run_undo(cx);
                        }
                    },
                )),
            )
            .child(
                primary_button(
                    t,
                    "rename",
                    format!("リネーム実行（{} 件）", s.ready),
                    can_execute,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run_rename(cx))),
            )
    }
}
