use std::ops::Range;

use gpui::{
    AnyElement, ClickEvent, Context, CursorStyle, FontWeight, MouseButton, MouseDownEvent, Pixels,
    Window, div, prelude::*, px, rgb, uniform_list,
};
use renamer_core::CandidateStatus;

use crate::app::{DragKind, RenamerView, ScrollTarget};
use crate::components::checkbox;
use crate::theme::Theme;
use crate::views::layout::SCROLLBAR;

const ROW_HEIGHT: f32 = 26.0;
const HEADER_HEIGHT: f32 = 30.0;
const CHECK_WIDTH: f32 = 28.0;
/// 列の区切り（ドラッグハンドル）の幅。行側も同じ幅の余白を入れて列をそろえる。
const HANDLE_WIDTH: f32 = 9.0;
const CELL_PADDING: f32 = 6.0;
/// 「場所」列の最小幅（これより狭くなると横スクロール）
const LOCATION_MIN_WIDTH: f32 = 200.0;

impl RenamerView {
    pub(crate) fn render_file_list(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = self.theme();
        let count = self.plan.len();
        let all_included = count > 0 && self.excluded.is_empty();
        let [w_name, w_new] = self.layout.column_widths.map(px);
        let focused = self.file_list_focus.is_focused(window);
        // 列の合計幅（右端にスクロールバーの分を足す）
        let content_width = CHECK_WIDTH
            + self.layout.column_widths[0]
            + self.layout.column_widths[1]
            + HANDLE_WIDTH * 2.0
            + LOCATION_MIN_WIDTH
            + SCROLLBAR;

        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .bg(rgb(t.bg))
            .border_b_1()
            .border_color(rgb(t.border))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(t.muted))
            .child(
                div()
                    .id("check-all")
                    .w(px(CHECK_WIDTH))
                    .flex_none()
                    .flex()
                    .justify_center()
                    .cursor_pointer()
                    .child(checkbox(t, all_included))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_all_excluded(all_included, cx)
                    })),
            )
            .child(column(w_name).child("現在の名前"))
            .child(self.resize_handle(t, 0, cx))
            .child(column(w_new).child("新しい名前"))
            .child(self.resize_handle(t, 1, cx))
            .child(rest_column().child("場所"));

        let body = if count == 0 {
            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_1()
                .items_center()
                .justify_center()
                .text_color(rgb(t.muted))
                .child("ここにファイルまたはフォルダをドロップして追加")
                .child(
                    div()
                        .text_xs()
                        .child("行をクリックで選択、Delete キーで一覧から削除"),
                )
                .into_any_element()
        } else {
            uniform_list(
                "file-list",
                count,
                cx.processor(move |this, range: Range<usize>, _window, cx| {
                    range
                        .map(|i| this.render_file_row(i, focused, cx))
                        .collect::<Vec<_>>()
                }),
            )
            .track_scroll(self.file_scroll.clone())
            .size_full()
            .into_any_element()
        };

        // 横スクロール（見出しと一覧が一緒に動く）
        let horizontal = div()
            .id("file-list-x")
            .size_full()
            .overflow_x_scroll()
            .track_scroll(&self.file_scroll_x)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .h_full()
                    .w_full()
                    .min_w(px(content_width))
                    .child(header)
                    .child(div().flex_1().min_h(px(0.0)).child(body)),
            );

        div()
            .track_focus(&self.file_list_focus)
            .key_context("FileList")
            .on_action(cx.listener(Self::remove_selected))
            .on_action(cx.listener(Self::select_all_files))
            .on_action(cx.listener(Self::clear_selection))
            .relative()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .overflow_hidden()
            .bg(rgb(t.panel))
            .child(horizontal)
            .child(self.scrollbar(ScrollTarget::FileList, HEADER_HEIGHT, cx))
            .child(self.scrollbar(ScrollTarget::FileListX, 0.0, cx))
    }

    /// 列の右端のドラッグハンドル。ドラッグ中の移動・終了はルート要素で処理する。
    fn resize_handle(&self, t: &Theme, col: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.drag.is_some_and(|d| d.kind == DragKind::Column(col));
        let accent = t.accent;
        div()
            .id(("col-resize", col))
            .flex_none()
            .w(px(HANDLE_WIDTH))
            .h_full()
            .flex()
            .justify_center()
            .cursor(CursorStyle::ResizeColumn)
            .child(
                div()
                    .w(px(if active { 2.0 } else { 1.0 }))
                    .h_full()
                    .bg(rgb(if active { t.accent } else { t.border }))
                    .hover(move |s| s.bg(rgb(accent))),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.begin_drag(DragKind::Column(col), event.position, cx);
                }),
            )
    }

    fn render_file_row(&self, i: usize, focused: bool, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme();
        let Some(item) = self.plan.items.get(i) else {
            return div().h(px(ROW_HEIGHT)).into_any_element();
        };
        let [w_name, w_new] = self.layout.column_widths.map(px);

        // 変更なし・除外はグレーアウト
        let greyed = matches!(
            item.status,
            CandidateStatus::Unchanged | CandidateStatus::Excluded
        );
        let excluded = item.status == CandidateStatus::Excluded;
        let is_error = item.status.is_error();
        let selected = self.selected_files.contains(&item.source);

        let name_color = if greyed { t.disabled } else { t.text };
        let new_color = if is_error {
            t.error
        } else if greyed {
            t.disabled
        } else {
            t.accent
        };
        let source_name = if item.is_dir {
            format!("{}/", item.source_name())
        } else {
            item.source_name()
        };
        let new_name = match (&item.status, item.status.error_message()) {
            // 詳細はルール欄とステータスバーに表示
            (CandidateStatus::RuleError, _) => "（ルールにエラーがあります）".to_string(),
            (_, Some(reason)) => format!("{}（{reason}）", item.new_name),
            _ => item.new_name.clone(),
        };
        let location = item
            .source
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        let bg = if selected {
            if focused { t.selected } else { t.hover }
        } else if is_error {
            t.error_bg
        } else {
            t.panel
        };
        let hover = t.hover;

        div()
            .id(("file-row", i))
            .flex()
            .items_center()
            .w_full()
            .h(px(ROW_HEIGHT))
            .border_b_1()
            .border_color(rgb(t.row_border))
            .bg(rgb(bg))
            .when(!selected, move |d| d.hover(move |s| s.bg(rgb(hover))))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.click_file(i, event.modifiers, window, cx)
                }),
            )
            .child(
                div()
                    .w(px(CHECK_WIDTH))
                    .h_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(checkbox(t, !excluded))
                    // チェックボックスのクリックは選択に影響させない
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.toggle_excluded(i, cx);
                        }),
                    ),
            )
            .child(
                column(w_name)
                    .text_color(rgb(name_color))
                    .child(source_name),
            )
            .child(spacer())
            .child(column(w_new).text_color(rgb(new_color)).child(new_name))
            .child(spacer())
            .child(
                rest_column()
                    .text_xs()
                    .text_color(rgb(if greyed { t.disabled } else { t.muted }))
                    .child(location),
            )
            .into_any_element()
    }
}

/// 固定幅の列。
fn column(width: Pixels) -> gpui::Div {
    text_cell().flex_none().w(width)
}

/// 残り幅をすべて使う列。
fn rest_column() -> gpui::Div {
    text_cell().flex_1()
}

/// はみ出した文字は切り詰める 1 行セル。
fn text_cell() -> gpui::Div {
    div()
        .min_w(px(0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .px(px(CELL_PADDING))
}

/// 行側でハンドルと同じ幅を確保する余白。
fn spacer() -> gpui::Div {
    div().flex_none().w(px(HANDLE_WIDTH))
}
