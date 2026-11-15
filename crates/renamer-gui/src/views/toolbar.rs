use std::rc::Rc;

use gpui::{ClickEvent, Context, div, prelude::*, px, rgb};
use renamer_core::{Platform, TargetType};

use crate::app::{DropdownKey, RenamerView};
use crate::components::{button, check_label};
use crate::theme::ThemeMode;
use crate::views::dropdown::labels;

impl RenamerView {
    pub(crate) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme();
        let input_label = match self.inputs.as_slice() {
            [] => "（未選択）ファイル/フォルダをドロップすると追加されます".to_string(),
            [one] => one.display().to_string(),
            many => format!("{} 件の入力（{} ほか）", many.len(), many[0].display()),
        };
        let has_input = !self.inputs.is_empty();

        let target_options: Vec<&'static str> = TargetType::ALL.iter().map(|v| v.label()).collect();
        let target_index = TargetType::ALL
            .iter()
            .position(|v| *v == self.scan_options.target)
            .unwrap_or(0);
        let theme_options: Vec<&'static str> = ThemeMode::ALL.iter().map(|v| v.label()).collect();
        let theme_index = ThemeMode::ALL
            .iter()
            .position(|v| *v == self.theme_mode)
            .unwrap_or(0);

        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(rgb(t.panel))
            .border_b_1()
            .border_color(rgb(t.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        button(t, "open-folder", "フォルダを開く", true).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.browse(true, cx)),
                        ),
                    )
                    .child(button(t, "open-files", "ファイルを選択", true).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.browse(false, cx)),
                    ))
                    .child(
                        button(t, "reload", "再読込", has_input).on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                if !this.inputs.is_empty() {
                                    this.rescan(cx);
                                }
                            },
                        )),
                    )
                    .child(
                        button(t, "clear", "クリア", has_input).on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                if !this.inputs.is_empty() {
                                    this.clear_inputs(cx);
                                }
                            },
                        )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_color(rgb(t.muted))
                            .child(input_label),
                    )
                    .child(div().flex_none().text_color(rgb(t.muted)).child("テーマ"))
                    .child(
                        self.dropdown(
                            DropdownKey::Theme,
                            labels(&theme_options),
                            theme_index,
                            Rc::new(|this, i, cx| {
                                if let Some(mode) = ThemeMode::ALL.get(i) {
                                    this.set_theme(*mode, cx);
                                }
                            }),
                            cx,
                        )
                        .w(px(120.0))
                        .flex_none(),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(div().flex_none().text_color(rgb(t.muted)).child("対象"))
                    .child(
                        self.dropdown(
                            DropdownKey::Target,
                            labels(&target_options),
                            target_index,
                            Rc::new(|this, i, cx| {
                                if let Some(target) = TargetType::ALL.get(i) {
                                    this.set_target(*target, cx);
                                }
                            }),
                            cx,
                        )
                        .w(px(170.0))
                        .flex_none(),
                    )
                    .child(
                        check_label(
                            t,
                            "opt-recursive",
                            "サブフォルダを含める",
                            self.scan_options.recursive,
                        )
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.scan_options.recursive = !this.scan_options.recursive;
                                this.rescan(cx);
                            },
                        )),
                    )
                    .child(
                        check_label(
                            t,
                            "opt-hidden",
                            "隠しファイルを含める",
                            self.scan_options.include_hidden,
                        )
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.scan_options.include_hidden =
                                    !this.scan_options.include_hidden;
                                this.rescan(cx);
                            },
                        )),
                    )
                    .child(
                        check_label(
                            t,
                            "opt-ext",
                            "拡張子もルールの対象にする",
                            self.plan_options.include_extension,
                        )
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.plan_options.include_extension =
                                    !this.plan_options.include_extension;
                                this.recompute(cx);
                            },
                        )),
                    )
                    .child(div().text_xs().text_color(rgb(t.muted)).child(format!(
                        "名前のチェック: {} の規則",
                        Platform::current().label()
                    ))),
            )
    }
}
