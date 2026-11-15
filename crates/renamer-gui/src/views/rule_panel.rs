use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Entity, FontWeight, Pixels, Point, SharedString, Window, div,
    prelude::*, px, rgb,
};
use renamer_core::{FieldValue, RenameRule, RuleField, parent_names};

use crate::app::{DropdownKey, Pane, RenamerView, ScrollTarget};
use crate::components::{button, check_label, checkbox, icon_button};
use crate::text_input::TextInput;
use crate::theme::Theme;
use crate::views::dropdown::{DROPDOWN_HEIGHT, labels};
use crate::views::layout::SCROLLBAR;

const LABEL_WIDTH: f32 = 120.0;

/// ドラッグ中のルール（ドラッグ中の見た目も兼ねる）。
#[derive(Clone)]
pub(crate) struct DraggedRule {
    index: usize,
    label: SharedString,
    position: Point<Pixels>,
    bg: u32,
    fg: u32,
    border: u32,
}

impl Render for DraggedRule {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // つかんだ位置にプレビューが来るようにずらす
        div().pl(self.position.x).pt(self.position.y).child(
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(rgb(self.border))
                .bg(rgb(self.bg))
                .text_color(rgb(self.fg))
                .text_sm()
                .shadow_md()
                .child(self.label.clone()),
        )
    }
}

impl RenamerView {
    pub(crate) fn render_rule_panel(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = self.theme();
        let l = self.layout;

        let rule_rows: Vec<AnyElement> = (0..self.rules.len())
            .map(|i| self.render_rule_row(t, i, cx))
            .collect();
        let rule_list = div()
            .flex()
            .flex_col()
            .gap_1()
            .py_1()
            .pr(px(SCROLLBAR))
            .when(self.rules.is_empty(), |d| {
                d.child(div().text_color(rgb(t.muted)).child("ルールがありません"))
            })
            .children(rule_rows);

        let content = div()
            .flex()
            .flex_col()
            .pr(px(SCROLLBAR))
            .pb_3()
            // ---- プリセット
            .child(section_title(t, "プリセット"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .pb_1()
                    .child(self.input_frame(t, &self.preset_name, false, window, cx))
                    .child(button(t, "preset-save", "保存", true).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.save_current_preset(cx)),
                    )),
            )
            .child(
                div().px_3().child(
                    self.scroll_area(ScrollTarget::Presets, self.render_preset_list(t, cx), cx)
                        .h(px(l.preset_height)),
                ),
            )
            .child(self.horizontal_splitter(Pane::Presets, cx))
            // ---- ルールを追加
            .child(section_title(t, "ルールを追加"))
            .child(self.render_add_buttons(t, cx))
            // ---- 適用するルール
            .child(
                div()
                    .flex()
                    .items_center()
                    .pr_3()
                    .child(div().flex_1().child(section_title(
                        t,
                        "適用するルール（上から順に適用・ドラッグで並べ替え）",
                    )))
                    .child(
                        button(t, "clear-rules", "すべて削除", !self.rules.is_empty()).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.clear_rules(cx)),
                        ),
                    ),
            )
            .child(
                div().px_3().child(
                    self.scroll_area(ScrollTarget::Rules, rule_list, cx)
                        .h(px(l.rule_list_height)),
                ),
            )
            .child(self.horizontal_splitter(Pane::Rules, cx))
            // ---- ルールの設定
            .child(section_title(t, "ルールの設定"))
            .child(
                div().px_3().child(
                    self.scroll_area(
                        ScrollTarget::Editor,
                        div()
                            .pr(px(SCROLLBAR + 2.0))
                            .pb_2()
                            .child(self.render_rule_editor(t, window, cx)),
                        cx,
                    )
                    .h(px(l.editor_height)),
                ),
            )
            .child(self.horizontal_splitter(Pane::Editor, cx));

        div()
            .w(px(l.left_width))
            .flex_none()
            .h_full()
            .bg(rgb(t.panel))
            .child(
                self.scroll_area(ScrollTarget::LeftPanel, content, cx)
                    .size_full(),
            )
    }

    fn render_preset_list(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<AnyElement> =
            self.presets
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let load_name = name.clone();
                    let delete_name = name.clone();
                    let hover = t.hover;
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .id(("preset", i))
                                .flex_1()
                                .min_w(px(0.0))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .cursor_pointer()
                                .hover(move |s| s.bg(rgb(hover)))
                                .child(name.clone())
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.load_named_preset(load_name.clone(), cx)
                                })),
                        )
                        .child(icon_button(t, ("preset-delete", i), "✕", true).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.delete_named_preset(delete_name.clone(), cx)
                            }),
                        ))
                        .into_any_element()
                })
                .collect();

        div()
            .flex()
            .flex_col()
            .pr(px(SCROLLBAR))
            .when(self.presets.is_empty(), |d| {
                d.child(
                    div()
                        .py_1()
                        .text_xs()
                        .text_color(rgb(t.muted))
                        .child("保存済みのプリセットはありません（クリックで読み込み）"),
                )
            })
            .children(rows)
            .into_any_element()
    }

    /// 分類ごとに 1 行ずつ並べる。
    fn render_add_buttons(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let groups: Vec<AnyElement> = RenameRule::template_groups()
            .into_iter()
            .enumerate()
            .map(|(gi, (group, templates))| {
                let buttons: Vec<_> = templates
                    .into_iter()
                    .enumerate()
                    .map(|(i, template)| {
                        button(t, ("add-rule", gi * 100 + i), template.kind_name(), true).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.add_rule(template.clone(), cx)
                            }),
                        )
                    })
                    .collect();
                div()
                    .flex()
                    .items_start()
                    .gap_2()
                    .child(
                        div()
                            .w(px(52.0))
                            .flex_none()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(t.muted))
                            .child(group),
                    )
                    .child(div().flex().flex_wrap().gap_1().children(buttons))
                    .into_any_element()
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .pb_2()
            .children(groups)
            .into_any_element()
    }

    fn render_rule_row(&self, t: &Theme, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let entry = &self.rules[i];
        let selected = self.selected_rule == Some(i);
        let has_error = self.plan.rule_error.as_ref().is_some_and(|e| e.index == i);
        let last = i + 1 == self.rules.len();
        let label = format!("{}. {}", i + 1, entry.rule.summary());

        let dragged = DraggedRule {
            index: i,
            label: label.clone().into(),
            position: Point::default(),
            bg: t.panel,
            fg: t.text,
            border: t.accent,
        };
        let (drop_bg, drop_border) = (t.accent_soft, t.accent);
        let border = if has_error {
            t.error
        } else if selected {
            t.accent
        } else {
            t.row_border
        };

        div()
            .id(("rule-row", i))
            .flex()
            .items_center()
            .gap_1()
            .px_1()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(if selected { t.selected } else { t.panel }))
            .on_drag(dragged, |dragged, position, _window, cx| {
                let mut preview = dragged.clone();
                preview.position = position;
                cx.new(|_| preview)
            })
            .drag_over::<DraggedRule>(move |style, _, _, _| {
                style.bg(rgb(drop_bg)).border_color(rgb(drop_border))
            })
            .on_drop(cx.listener(move |this, dragged: &DraggedRule, _, cx| {
                this.reorder_rule(dragged.index, i, cx)
            }))
            .child(
                // つかむ場所の目印
                div()
                    .flex_none()
                    .px_1()
                    .text_color(rgb(t.disabled))
                    .cursor_move()
                    .child("⋮⋮"),
            )
            .child(
                div()
                    .id(("rule-enable", i))
                    .p_1()
                    .cursor_pointer()
                    .child(checkbox(t, entry.enabled))
                    .on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_rule(i, cx)),
                    ),
            )
            .child(
                div()
                    .id(("rule-label", i))
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .text_color(rgb(if has_error {
                        t.error
                    } else if entry.enabled {
                        t.text
                    } else {
                        t.disabled
                    }))
                    .child(if has_error {
                        format!("⚠ {label}")
                    } else {
                        label
                    })
                    .on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.select_rule(i, cx)),
                    ),
            )
            .child(
                icon_button(t, ("rule-up", i), "↑", i > 0).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        if i > 0 {
                            this.reorder_rule(i, i - 1, cx)
                        }
                    },
                )),
            )
            .child(icon_button(t, ("rule-down", i), "↓", !last).on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| this.reorder_rule(i, i + 1, cx)),
            ))
            .child(
                icon_button(t, ("rule-remove", i), "✕", true).on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.remove_rule(i, cx)),
                ),
            )
            .into_any_element()
    }

    fn render_rule_editor(
        &self,
        t: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some((index, entry)) = self
            .selected_rule
            .and_then(|i| self.rules.get(i).map(|e| (i, e)))
        else {
            return div()
                .text_color(rgb(t.muted))
                .child("ルールをクリックすると設定を編集できます")
                .into_any_element();
        };
        let rule_error = self
            .plan
            .rule_error
            .as_ref()
            .filter(|e| e.index == index)
            .map(|e| e.message.clone());
        let is_regex = matches!(entry.rule, RenameRule::RegexReplace { .. });

        let fields = entry.rule.fields();
        let uses_tokens = fields.iter().any(|f| f.accepts_tokens);
        let rows: Vec<AnyElement> = fields
            .into_iter()
            .enumerate()
            .map(|(fi, field)| self.render_field(t, fi, field, window, cx))
            .collect();

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(entry.rule.kind_name()),
            )
            .children(rows)
            .when_some(rule_error, |d, message| {
                d.child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(t.error_bg))
                        .text_color(rgb(t.error))
                        .child(message),
                )
            })
            .when(!self.invalid_fields.is_empty(), |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(rgb(t.error))
                        .child("数値として解釈できない値があります（反映されていません）"),
                )
            })
            .when(is_regex, |d| {
                d.child(hint(
                    t,
                    "置換後には $1〜$9 で ( ) のグループを挿入できます（$$ で $ 1 文字）",
                ))
            })
            .when(uses_tokens, |d| d.child(self.token_hint(t)))
            .into_any_element()
    }

    /// `\f0`〜`\f9` の説明と、一覧の先頭の項目での例。
    fn token_hint(&self, t: &Theme) -> AnyElement {
        let example = self.plan.items.first().map(|item| {
            let parents = parent_names(&item.source);
            let shown: Vec<String> = parents
                .iter()
                .take(3)
                .enumerate()
                .map(|(i, name)| format!("\\f{i} → 「{name}」"))
                .collect();
            format!("例（{}）: {}", item.source_name(), shown.join("、"))
        });
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(hint(
                t,
                "\\f0〜\\f9 で親フォルダ名を挿入できます（\\f0 = 入っているフォルダ、\\f1 = その 1 つ上 …）。\\\\ で \\ 1 文字",
            ))
            .when_some(example, |d, text| d.child(hint(t, text)))
            .into_any_element()
    }

    fn render_field(
        &self,
        t: &Theme,
        fi: usize,
        field: RuleField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = div()
            .w(px(LABEL_WIDTH))
            .flex_none()
            .text_color(rgb(t.muted));

        let row = div()
            .flex()
            .items_center()
            .gap_2()
            .min_h(px(DROPDOWN_HEIGHT));
        match field.value {
            FieldValue::Text(_) | FieldValue::Number(_) => {
                let Some(input) = self.field_inputs.get(fi).cloned().flatten() else {
                    return div().into_any_element();
                };
                row.child(label.child(field.label))
                    .child(self.input_frame(
                        t,
                        &input,
                        self.invalid_fields.contains(&fi),
                        window,
                        cx,
                    ))
                    .into_any_element()
            }
            FieldValue::Choice { options, selected } => row
                .child(label.child(field.label))
                .child(
                    self.dropdown(
                        DropdownKey::Field(fi),
                        labels(&options),
                        selected,
                        Rc::new(move |this, i, cx| this.set_field_choice(fi, i, cx)),
                        cx,
                    )
                    .flex_1()
                    .min_w(px(0.0)),
                )
                .into_any_element(),
            FieldValue::Bool(on) => row
                .child(label)
                .child(
                    check_label(t, ("field-bool", fi), field.label, on).on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| this.toggle_field_bool(fi, cx),
                    )),
                )
                .into_any_element(),
        }
    }

    /// テキスト入力の枠（フォーカス時はアクセント色、不正値は赤）。
    fn input_frame(
        &self,
        t: &Theme,
        input: &Entity<TextInput>,
        invalid: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let focused = input.read(cx).focus_handle.is_focused(window);
        let border = if invalid {
            t.error
        } else if focused {
            t.accent
        } else {
            t.border
        };
        div()
            .flex_1()
            .min_w(px(0.0))
            .h(px(DROPDOWN_HEIGHT))
            .px_2()
            .flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(t.input_bg))
            .text_color(rgb(t.text))
            .child(input.clone())
    }
}

fn section_title(t: &Theme, text: &'static str) -> impl IntoElement {
    div()
        .px_3()
        .pt_3()
        .pb_1()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(t.muted))
        .child(text)
}

fn hint(t: &Theme, text: impl Into<SharedString>) -> impl IntoElement {
    div().text_xs().text_color(rgb(t.muted)).child(text.into())
}
