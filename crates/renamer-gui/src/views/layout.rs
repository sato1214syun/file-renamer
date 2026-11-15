//! 境界のドラッグ（サイズ変更）とスクロールバー。

use gpui::{
    AnyElement, Context, CursorStyle, Div, MouseButton, MouseDownEvent, Pixels, Point, Size, div,
    prelude::*, px, rgb,
};

use crate::app::{DragKind, Pane, RenamerView, ScrollTarget};

/// 境界の当たり判定の太さ
const SPLITTER: f32 = 6.0;
/// スクロールバーの太さ
pub(crate) const SCROLLBAR: f32 = 10.0;
const MIN_THUMB: f32 = 24.0;

/// `ScrollHandle::max_offset()` の戻り値の型が GPUI のバージョンで
/// `Size` / `Point` と異なるため、どちらでも (x, y) を取り出せるようにする。
trait AxisPair {
    fn pair(&self) -> (f32, f32);
}

impl AxisPair for Size<Pixels> {
    fn pair(&self) -> (f32, f32) {
        (f32::from(self.width), f32::from(self.height))
    }
}

impl AxisPair for Point<Pixels> {
    fn pair(&self) -> (f32, f32) {
        (f32::from(self.x), f32::from(self.y))
    }
}

impl RenamerView {
    /// 左パネルと一覧の間の縦の境界（左右にドラッグして幅を変える）。
    pub(crate) fn vertical_splitter(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme();
        let active = matches!(self.drag.map(|d| d.kind), Some(DragKind::LeftPanel));
        let accent = t.accent;
        div()
            .id("split-left")
            .flex_none()
            .w(px(SPLITTER))
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
                cx.listener(|this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.begin_drag(DragKind::LeftPanel, e.position, cx);
                }),
            )
    }

    /// 領域の下端の横の境界（上下にドラッグして高さを変える）。
    pub(crate) fn horizontal_splitter(
        &self,
        pane: Pane,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = self.theme();
        let active = self.drag.is_some_and(|d| d.kind == DragKind::Pane(pane));
        let accent = t.accent;
        div()
            .id(("split-pane", pane as usize))
            .flex_none()
            .w_full()
            .h(px(SPLITTER))
            .flex()
            .flex_col()
            .justify_center()
            .cursor(CursorStyle::ResizeRow)
            .child(
                div()
                    .h(px(if active { 2.0 } else { 1.0 }))
                    .w_full()
                    .bg(rgb(if active { t.accent } else { t.border }))
                    .hover(move |s| s.bg(rgb(accent))),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.begin_drag(DragKind::Pane(pane), e.position, cx);
                }),
            )
    }

    /// 縦スクロールできる領域（右端にスクロールバー付き）。高さは呼び出し側で指定する。
    pub(crate) fn scroll_area(
        &self,
        target: ScrollTarget,
        content: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .relative()
            .overflow_hidden()
            .child(
                div()
                    .id(("scroll-area", target.index()))
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle(target))
                    .child(content),
            )
            .child(self.scrollbar(target, 0.0, cx))
    }

    /// スクロールバー。親は `relative()` で、スクロール領域と同じ位置・大きさであること。
    /// `start` は縦なら上端、横なら左端からのずれ（見出し行の高さなど）。
    pub(crate) fn scrollbar(
        &self,
        target: ScrollTarget,
        start: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = self.theme();
        let handle = self.scroll_handle(target);
        let vertical = target.is_vertical();
        let viewport = handle.bounds().size;
        let (max_x, max_y) = handle.max_offset().pair();
        let offset = handle.offset();

        let (view_len, max, pos) = if vertical {
            (f32::from(viewport.height), max_y, -f32::from(offset.y))
        } else {
            (f32::from(viewport.width), max_x, -f32::from(offset.x))
        };
        if view_len <= MIN_THUMB || max <= 0.5 {
            return div().into_any_element();
        }

        let thumb_len = (view_len * view_len / (view_len + max)).clamp(MIN_THUMB, view_len);
        let travel = (view_len - thumb_len).max(1.0);
        let pos = pos.clamp(0.0, max);
        let thumb_start = travel * pos / max;
        let ratio = max / travel;

        let dragging = self
            .drag
            .is_some_and(|d| matches!(d.kind, DragKind::Thumb { target: tg, .. } if tg == target));
        let color = if dragging {
            t.scrollbar_hover
        } else {
            t.scrollbar
        };
        let hover = t.scrollbar_hover;

        let thumb = div()
            .id(("scroll-thumb", target.index()))
            .absolute()
            .rounded_full()
            .bg(rgb(color))
            .hover(move |s| s.bg(rgb(hover)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.begin_drag(DragKind::Thumb { target, ratio, max }, e.position, cx);
                }),
            );
        let thumb = if vertical {
            thumb
                .top(px(thumb_start))
                .left(px(2.0))
                .w(px(SCROLLBAR - 4.0))
                .h(px(thumb_len))
        } else {
            thumb
                .left(px(thumb_start))
                .top(px(2.0))
                .h(px(SCROLLBAR - 4.0))
                .w(px(thumb_len))
        };

        let track = div().absolute();
        let track = if vertical {
            track
                .right_0()
                .top(px(start))
                .w(px(SCROLLBAR))
                .h(px(view_len))
        } else {
            track
                .bottom_0()
                .left(px(start))
                .h(px(SCROLLBAR))
                .w(px(view_len))
        };
        track.child(thumb).into_any_element()
    }
}
