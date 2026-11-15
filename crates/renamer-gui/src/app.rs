//! アプリ全体の状態と操作。描画は `views` モジュールに分割している。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{
    App, Context, CursorStyle, Entity, ExternalPaths, FocusHandle, KeyBinding, Modifiers,
    MouseButton, MouseMoveEvent, MouseUpEvent, PathPromptOptions, Pixels, Point, ScrollHandle,
    Subscription, UniformListScrollHandle, Window, WindowAppearance, actions, div, point,
    prelude::*, px, rgb,
};
use renamer_core::{
    FieldValue, FileEntry, PlanOptions, RenamePlan, RenameRule, RuleEntry, TargetType, build_plan,
};
use renamer_fs::{
    Operation, Preset, ScanOptions, UndoJournal, delete_preset, execute_plan, list_presets,
    load_preset, save_preset, scan_inputs, undo,
};

use crate::session::{Layout, Session};
use crate::text_input::{TextInput, TextInputEvent};
use crate::theme::{DARK, LIGHT, Theme, ThemeMode};

actions!(file_list, [RemoveSelected, SelectAllFiles, ClearSelection]);

const FILE_LIST_CONTEXT: &str = "FileList";

/// 一覧用のキーバインドを登録する（起動時に 1 回）。
pub fn bind_keys(cx: &mut App) {
    let ctx = Some(FILE_LIST_CONTEXT);
    #[cfg(target_os = "macos")]
    let select_all = "cmd-a";
    #[cfg(not(target_os = "macos"))]
    let select_all = "ctrl-a";
    cx.bind_keys([
        KeyBinding::new("delete", RemoveSelected, ctx),
        // macOS の delete キーは backspace として届く
        KeyBinding::new("backspace", RemoveSelected, ctx),
        KeyBinding::new(select_all, SelectAllFiles, ctx),
        KeyBinding::new("escape", ClearSelection, ctx),
    ]);
}

// ---------------------------------------------------------------- サイズの制限

pub(crate) const MIN_COLUMN_WIDTH: f32 = 80.0;
pub(crate) const MIN_LEFT_WIDTH: f32 = 280.0;
/// 右側の一覧に最低限残す幅
pub(crate) const MIN_RIGHT_WIDTH: f32 = 300.0;
pub(crate) const MIN_PANE_HEIGHT: f32 = 48.0;

/// 高さを変えられる左パネル内の領域
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pane {
    Presets,
    Rules,
    Editor,
}

/// スクロールできる領域
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrollTarget {
    LeftPanel,
    Presets,
    Rules,
    Editor,
    FileList,
    FileListX,
}

impl ScrollTarget {
    pub fn is_vertical(self) -> bool {
        self != Self::FileListX
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// ドロップダウンの識別子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DropdownKey {
    /// 選択中ルールの n 番目の項目
    Field(usize),
    Target,
    Theme,
}

impl DropdownKey {
    pub fn index(self) -> usize {
        match self {
            Self::Field(i) => i,
            Self::Target => 90,
            Self::Theme => 91,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DragKind {
    /// 一覧の列幅
    Column(usize),
    /// 左パネルの幅
    LeftPanel,
    /// 左パネル内の領域の高さ
    Pane(Pane),
    /// スクロールバーのつまみ（ratio = 1px 動かしたときのスクロール量）
    Thumb {
        target: ScrollTarget,
        ratio: f32,
        max: f32,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ActiveDrag {
    pub kind: DragKind,
    pub start: Point<Pixels>,
    pub start_value: f32,
}

pub struct RenamerView {
    // ---- 入力・走査
    /// ユーザーが指定したフォルダ・ファイル
    pub(crate) inputs: Vec<PathBuf>,
    pub(crate) scan_options: ScanOptions,
    pub(crate) plan_options: PlanOptions,
    pub(crate) entries: Vec<FileEntry>,
    /// 一覧から削除した項目（再読込しても表示しない）
    pub(crate) removed: HashSet<PathBuf>,
    /// 一覧でチェックを外した項目
    pub(crate) excluded: HashSet<PathBuf>,

    // ---- 一覧の選択
    pub(crate) file_list_focus: FocusHandle,
    pub(crate) selected_files: HashSet<PathBuf>,
    /// Shift+クリックの起点
    selection_anchor: Option<usize>,

    // ---- ルール
    pub(crate) rules: Vec<RuleEntry>,
    pub(crate) selected_rule: Option<usize>,
    /// 選択中ルールの各フィールドの入力欄（テキスト/数値以外は None）
    pub(crate) field_inputs: Vec<Option<Entity<TextInput>>>,
    /// 数値として解釈できない入力があるフィールド
    pub(crate) invalid_fields: HashSet<usize>,
    field_subscriptions: Vec<Subscription>,

    // ---- プリセット
    pub(crate) preset_name: Entity<TextInput>,
    pub(crate) presets: Vec<String>,

    // ---- 表示
    pub(crate) theme_mode: ThemeMode,
    pub(crate) dark: bool,
    pub(crate) layout: Layout,
    pub(crate) drag: Option<ActiveDrag>,
    pub(crate) open_dropdown: Option<DropdownKey>,
    /// 外側クリックで閉じた直後に、同じボタンのクリックで再び開かないようにする
    dropdown_closed: Option<(DropdownKey, Instant)>,

    // ---- スクロール
    pub(crate) left_scroll: ScrollHandle,
    pub(crate) preset_scroll: ScrollHandle,
    pub(crate) rule_scroll: ScrollHandle,
    pub(crate) editor_scroll: ScrollHandle,
    pub(crate) file_scroll: UniformListScrollHandle,
    pub(crate) file_scroll_x: ScrollHandle,

    // ---- 結果
    pub(crate) plan: RenamePlan,
    pub(crate) status: String,
    pub(crate) status_is_error: bool,
    pub(crate) can_undo: bool,

    last_session_json: String,
    _appearance_subscription: Subscription,
}

impl RenamerView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let session = Session::load().unwrap_or_default();
        let preset_name = cx.new(|cx| TextInput::new(cx, "", "プリセット名"));
        let appearance = cx.observe_window_appearance(window, |_, _, cx| cx.notify());

        let mut this = Self {
            inputs: Vec::new(),
            scan_options: session.preset.scan_options,
            plan_options: session.preset.plan_options,
            entries: Vec::new(),
            removed: HashSet::new(),
            excluded: HashSet::new(),
            file_list_focus: cx.focus_handle(),
            selected_files: HashSet::new(),
            selection_anchor: None,
            rules: session.preset.rules.clone(),
            selected_rule: None,
            field_inputs: Vec::new(),
            invalid_fields: HashSet::new(),
            field_subscriptions: Vec::new(),
            preset_name,
            presets: list_presets(),
            theme_mode: session.theme,
            dark: false,
            layout: sanitize_layout(session.layout),
            drag: None,
            open_dropdown: None,
            dropdown_closed: None,
            left_scroll: ScrollHandle::new(),
            preset_scroll: ScrollHandle::new(),
            rule_scroll: ScrollHandle::new(),
            editor_scroll: ScrollHandle::new(),
            file_scroll: UniformListScrollHandle::new(),
            file_scroll_x: ScrollHandle::new(),
            plan: RenamePlan::default(),
            status: "フォルダを開くか、ファイル/フォルダをウィンドウにドロップしてください".into(),
            status_is_error: false,
            can_undo: UndoJournal::load_default().is_some_and(|j| !j.is_empty()),
            last_session_json: session.to_json().unwrap_or_default(),
            _appearance_subscription: appearance,
        };
        if !this.rules.is_empty() {
            this.selected_rule = Some(0);
            this.rebuild_field_inputs(cx);
        }
        this.recompute(cx);
        this
    }

    pub(crate) fn theme(&self) -> &'static Theme {
        if self.dark { &DARK } else { &LIGHT }
    }

    // ---------------------------------------------------------------- 入力・走査

    /// 対象を入れ替える（フォルダ/ファイル選択ダイアログ）。
    pub(crate) fn set_inputs(&mut self, inputs: Vec<PathBuf>, cx: &mut Context<Self>) {
        if inputs.is_empty() {
            return;
        }
        self.inputs = inputs;
        self.removed.clear();
        self.excluded.clear();
        self.clear_file_selection();
        self.rescan(cx);
    }

    /// 対象を追加する（ドラッグ＆ドロップ）。
    pub(crate) fn add_inputs(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let before = self.entries.len();
        for path in paths {
            // 以前に一覧から削除したものを再度追加した場合は復活させる
            self.removed.retain(|r| !r.starts_with(&path));
            if !self.inputs.contains(&path) {
                self.inputs.push(path);
            }
        }
        self.rescan(cx);
        if !self.status_is_error {
            let added = self.entries.len().saturating_sub(before);
            self.set_status(
                format!("{added} 件を追加しました（合計 {} 件）", self.entries.len()),
                false,
            );
        }
    }

    pub(crate) fn clear_inputs(&mut self, cx: &mut Context<Self>) {
        self.inputs.clear();
        self.entries.clear();
        self.removed.clear();
        self.excluded.clear();
        self.clear_file_selection();
        self.set_status("一覧をクリアしました", false);
        self.recompute(cx);
    }

    pub(crate) fn browse(&mut self, directories: bool, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: !directories,
            directories,
            multiple: !directories,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            this.update(cx, |this, cx| this.set_inputs(paths, cx)).ok();
        })
        .detach();
    }

    pub(crate) fn rescan(&mut self, cx: &mut Context<Self>) {
        if self.inputs.is_empty() {
            self.entries.clear();
            self.recompute(cx);
            return;
        }
        // 削除済み・存在しなくなった入力は無視する
        let inputs: Vec<PathBuf> = self
            .inputs
            .iter()
            .filter(|p| !self.removed.contains(*p) && p.exists())
            .cloned()
            .collect();
        match scan_inputs(&inputs, &self.scan_options) {
            Ok(entries) => {
                self.entries = entries
                    .into_iter()
                    .filter(|e| !self.removed.contains(&e.path))
                    .collect();
                self.set_status(format!("{} 件を読み込みました", self.entries.len()), false);
            }
            Err(err) => {
                self.entries.clear();
                self.set_status(format!("読み込みに失敗しました: {err}"), true);
            }
        }
        // 一覧から消えた項目の選択は解除
        let present: HashSet<&PathBuf> = self.entries.iter().map(|e| &e.path).collect();
        self.selected_files.retain(|p| present.contains(p));
        self.recompute(cx);
    }

    /// ルールやオプションの変更後にプレビューを作り直す（状態も自動保存）。
    pub(crate) fn recompute(&mut self, cx: &mut Context<Self>) {
        self.plan = build_plan(
            &self.entries,
            &self.rules,
            &self.plan_options,
            &self.excluded,
            |p| p.symlink_metadata().is_ok(),
        );
        self.save_session();
        cx.notify();
    }

    pub(crate) fn toggle_excluded(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(item) = self.plan.items.get(index) else {
            return;
        };
        let path = item.source.clone();
        if !self.excluded.remove(&path) {
            self.excluded.insert(path);
        }
        self.recompute(cx);
    }

    pub(crate) fn set_all_excluded(&mut self, exclude: bool, cx: &mut Context<Self>) {
        if exclude {
            self.excluded = self.entries.iter().map(|e| e.path.clone()).collect();
        } else {
            self.excluded.clear();
        }
        self.recompute(cx);
    }

    pub(crate) fn set_target(&mut self, target: TargetType, cx: &mut Context<Self>) {
        if self.scan_options.target != target {
            self.scan_options.target = target;
            self.excluded.clear();
            self.rescan(cx);
        }
    }

    // ---------------------------------------------------------------- 一覧の選択・削除

    /// 行クリック。Ctrl(⌘) で追加/解除、Shift で範囲選択。
    pub(crate) fn click_file(
        &mut self,
        index: usize,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.file_list_focus);
        let Some(path) = self.plan.items.get(index).map(|c| c.source.clone()) else {
            return;
        };
        let toggle = modifiers.control || modifiers.platform;
        match self.selection_anchor.filter(|_| modifiers.shift) {
            Some(anchor) => {
                let anchor = anchor.min(self.plan.items.len().saturating_sub(1));
                let (lo, hi) = (anchor.min(index), anchor.max(index));
                if !toggle {
                    self.selected_files.clear();
                }
                self.selected_files
                    .extend(self.plan.items[lo..=hi].iter().map(|c| c.source.clone()));
            }
            None => {
                if toggle {
                    if !self.selected_files.remove(&path) {
                        self.selected_files.insert(path);
                    }
                } else {
                    self.selected_files.clear();
                    self.selected_files.insert(path);
                }
                self.selection_anchor = Some(index);
            }
        }
        cx.notify();
    }

    fn clear_file_selection(&mut self) {
        self.selected_files.clear();
        self.selection_anchor = None;
    }

    pub(crate) fn remove_selected(
        &mut self,
        _: &RemoveSelected,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_files.is_empty() {
            return;
        }
        let count = self.selected_files.len();
        let selected = std::mem::take(&mut self.selected_files);
        self.entries.retain(|e| !selected.contains(&e.path));
        self.excluded.retain(|p| !selected.contains(p));
        self.removed.extend(selected);
        self.selection_anchor = None;
        self.set_status(
            format!("{count} 件を一覧から削除しました（ファイルは削除されません）"),
            false,
        );
        self.recompute(cx);
    }

    pub(crate) fn select_all_files(
        &mut self,
        _: &SelectAllFiles,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_files = self.entries.iter().map(|e| e.path.clone()).collect();
        cx.notify();
    }

    pub(crate) fn clear_selection(
        &mut self,
        _: &ClearSelection,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_file_selection();
        cx.notify();
    }

    // ---------------------------------------------------------------- ルール

    pub(crate) fn add_rule(&mut self, rule: RenameRule, cx: &mut Context<Self>) {
        self.rules.push(RuleEntry::new(rule));
        self.selected_rule = Some(self.rules.len() - 1);
        self.rebuild_field_inputs(cx);
        self.recompute(cx);
        // 追加したルールが見えるように一覧の末尾へ
        self.rule_scroll.scroll_to_bottom();
    }

    pub(crate) fn select_rule(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.selected_rule != Some(index) {
            self.selected_rule = Some(index);
            self.rebuild_field_inputs(cx);
        }
        cx.notify();
    }

    pub(crate) fn toggle_rule(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(entry) = self.rules.get_mut(index) {
            entry.enabled = !entry.enabled;
            self.recompute(cx);
        }
    }

    pub(crate) fn remove_rule(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.rules.len() {
            return;
        }
        self.rules.remove(index);
        self.selected_rule = match self.selected_rule {
            Some(s) if s == index => None,
            Some(s) if s > index => Some(s - 1),
            other => other,
        };
        self.rebuild_field_inputs(cx);
        self.recompute(cx);
    }

    pub(crate) fn clear_rules(&mut self, cx: &mut Context<Self>) {
        self.rules.clear();
        self.selected_rule = None;
        self.rebuild_field_inputs(cx);
        self.recompute(cx);
    }

    /// ドラッグ＆ドロップでルールを `from` から `to` の位置へ移動する。
    pub(crate) fn reorder_rule(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let len = self.rules.len();
        if from >= len || from == to {
            return;
        }
        let to = to.min(len - 1);
        let mut order: Vec<usize> = (0..len).collect();
        let moved = order.remove(from);
        order.insert(to, moved);

        let rule = self.rules.remove(from);
        self.rules.insert(to, rule);
        // 選択中のルールは同じルールを指し続ける（入力欄も作り直さない）
        self.selected_rule = self
            .selected_rule
            .and_then(|s| order.iter().position(|&o| o == s));
        self.recompute(cx);
    }

    pub(crate) fn set_field_choice(
        &mut self,
        field: usize,
        selected: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.selected_rule.and_then(|i| self.rules.get_mut(i)) else {
            return;
        };
        if entry.rule.set_choice(field, selected) {
            self.recompute(cx);
        }
    }

    pub(crate) fn toggle_field_bool(&mut self, field: usize, cx: &mut Context<Self>) {
        let Some(entry) = self.selected_rule.and_then(|i| self.rules.get_mut(i)) else {
            return;
        };
        let current = match entry.rule.fields().get(field).map(|f| &f.value) {
            Some(FieldValue::Bool(on)) => *on,
            _ => return,
        };
        if entry.rule.set_bool(field, !current) {
            self.recompute(cx);
        }
    }

    /// 選択中ルールの入力欄を作り直す。
    fn rebuild_field_inputs(&mut self, cx: &mut Context<Self>) {
        self.field_inputs.clear();
        self.field_subscriptions.clear();
        self.invalid_fields.clear();
        self.open_dropdown = None;

        let fields = match self.selected_rule.and_then(|i| self.rules.get(i)) {
            Some(entry) => entry.rule.fields(),
            None => return,
        };
        for (fi, field) in fields.into_iter().enumerate() {
            let (value, placeholder) = match field.value {
                FieldValue::Text(v) => (v, "（空）"),
                FieldValue::Number(v) => (v, "0"),
                _ => {
                    self.field_inputs.push(None);
                    continue;
                }
            };
            let input = cx.new(|cx| TextInput::new(cx, value, placeholder));
            let subscription = cx.subscribe(
                &input,
                move |this: &mut Self, input: Entity<TextInput>, _: &TextInputEvent, cx| {
                    this.on_field_changed(fi, input, cx);
                },
            );
            self.field_inputs.push(Some(input));
            self.field_subscriptions.push(subscription);
        }
    }

    fn on_field_changed(&mut self, field: usize, input: Entity<TextInput>, cx: &mut Context<Self>) {
        let text = input.read(cx).text().to_string();
        let Some(entry) = self.selected_rule.and_then(|i| self.rules.get_mut(i)) else {
            return;
        };
        if entry.rule.set_field(field, &text) {
            self.invalid_fields.remove(&field);
            self.recompute(cx);
        } else {
            self.invalid_fields.insert(field);
            cx.notify();
        }
    }

    // ---------------------------------------------------------------- ドロップダウン

    pub(crate) fn toggle_dropdown(&mut self, key: DropdownKey, cx: &mut Context<Self>) {
        if self.open_dropdown == Some(key) {
            self.open_dropdown = None;
        } else if self
            .dropdown_closed
            .is_some_and(|(k, at)| k == key && at.elapsed() < Duration::from_millis(400))
        {
            // 開いているメニューの外（＝このボタン）をクリックして閉じた直後
            self.dropdown_closed = None;
        } else {
            self.open_dropdown = Some(key);
        }
        cx.notify();
    }

    /// メニューの外側がクリックされたとき。
    pub(crate) fn close_dropdown(&mut self, key: DropdownKey, cx: &mut Context<Self>) {
        if self.open_dropdown == Some(key) {
            self.open_dropdown = None;
            self.dropdown_closed = Some((key, Instant::now()));
            cx.notify();
        }
    }

    // ---------------------------------------------------------------- プリセット / セッション

    fn current_preset(&self) -> Preset {
        Preset {
            rules: self.rules.clone(),
            plan_options: self.plan_options,
            scan_options: self.scan_options,
            ..Default::default()
        }
    }

    pub(crate) fn save_current_preset(&mut self, cx: &mut Context<Self>) {
        let name = self.preset_name.read(cx).text().trim().to_string();
        if name.is_empty() {
            self.set_status("プリセット名を入力してください", true);
            cx.notify();
            return;
        }
        let overwrite = self.presets.contains(&name);
        match save_preset(&name, &self.current_preset()) {
            Ok(()) => self.set_status(
                if overwrite {
                    format!("プリセット「{name}」を上書き保存しました")
                } else {
                    format!("プリセット「{name}」を保存しました")
                },
                false,
            ),
            Err(err) => self.set_status(format!("保存できませんでした: {err}"), true),
        }
        self.presets = list_presets();
        cx.notify();
    }

    pub(crate) fn load_named_preset(&mut self, name: String, cx: &mut Context<Self>) {
        match load_preset(&name) {
            Ok(preset) => {
                self.rules = preset.rules;
                self.plan_options = preset.plan_options;
                self.scan_options = preset.scan_options;
                self.selected_rule = (!self.rules.is_empty()).then_some(0);
                self.excluded.clear();
                self.rebuild_field_inputs(cx);
                let label = name.clone();
                self.preset_name
                    .update(cx, |input, cx| input.set_text(label, cx));
                self.rescan(cx);
                self.set_status(format!("プリセット「{name}」を読み込みました"), false);
            }
            Err(err) => self.set_status(format!("読み込めませんでした: {err}"), true),
        }
        cx.notify();
    }

    pub(crate) fn delete_named_preset(&mut self, name: String, cx: &mut Context<Self>) {
        match delete_preset(&name) {
            Ok(()) => self.set_status(format!("プリセット「{name}」を削除しました"), false),
            Err(err) => self.set_status(format!("削除できませんでした: {err}"), true),
        }
        self.presets = list_presets();
        cx.notify();
    }

    /// 現在の状態を session.json に保存する（内容が変わったときだけ書き込む）。
    pub(crate) fn save_session(&mut self) {
        let session = Session {
            preset: self.current_preset(),
            theme: self.theme_mode,
            layout: self.layout,
        };
        let Some(json) = session.to_json() else {
            return;
        };
        if json == self.last_session_json {
            return;
        }
        match Session::write_json(&json) {
            Ok(()) => self.last_session_json = json,
            Err(err) => tracing::warn!("セッションを保存できません: {err}"),
        }
    }

    pub(crate) fn set_theme(&mut self, mode: ThemeMode, cx: &mut Context<Self>) {
        self.theme_mode = mode;
        self.save_session();
        cx.notify();
    }

    // ---------------------------------------------------------------- スクロール

    pub(crate) fn scroll_handle(&self, target: ScrollTarget) -> ScrollHandle {
        match target {
            ScrollTarget::LeftPanel => self.left_scroll.clone(),
            ScrollTarget::Presets => self.preset_scroll.clone(),
            ScrollTarget::Rules => self.rule_scroll.clone(),
            ScrollTarget::Editor => self.editor_scroll.clone(),
            ScrollTarget::FileList => self.file_scroll.0.borrow().base_handle.clone(),
            ScrollTarget::FileListX => self.file_scroll_x.clone(),
        }
    }

    // ---------------------------------------------------------------- 境界・つまみのドラッグ

    pub(crate) fn begin_drag(
        &mut self,
        kind: DragKind,
        start: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let start_value = match kind {
            DragKind::Column(c) => self.layout.column_widths[c],
            DragKind::LeftPanel => self.layout.left_width,
            DragKind::Pane(Pane::Presets) => self.layout.preset_height,
            DragKind::Pane(Pane::Rules) => self.layout.rule_list_height,
            DragKind::Pane(Pane::Editor) => self.layout.editor_height,
            DragKind::Thumb { target, .. } => {
                let offset = self.scroll_handle(target).offset();
                -f32::from(if target.is_vertical() {
                    offset.y
                } else {
                    offset.x
                })
            }
        };
        self.drag = Some(ActiveDrag {
            kind,
            start,
            start_value,
        });
        self.open_dropdown = None;
        cx.notify();
    }

    fn on_root_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.drag else {
            return;
        };
        // ウィンドウ外でボタンを離した場合
        if event.pressed_button != Some(MouseButton::Left) {
            self.end_drag(cx);
            return;
        }
        let dx = f32::from(event.position.x - drag.start.x);
        let dy = f32::from(event.position.y - drag.start.y);
        let l = &mut self.layout;
        match drag.kind {
            DragKind::Column(c) => {
                l.column_widths[c] = (drag.start_value + dx).max(MIN_COLUMN_WIDTH);
            }
            DragKind::LeftPanel => {
                let max =
                    (f32::from(window.viewport_size().width) - MIN_RIGHT_WIDTH).max(MIN_LEFT_WIDTH);
                l.left_width = (drag.start_value + dx).clamp(MIN_LEFT_WIDTH, max);
            }
            DragKind::Pane(pane) => {
                let h = (drag.start_value + dy).max(MIN_PANE_HEIGHT);
                match pane {
                    Pane::Presets => l.preset_height = h,
                    Pane::Rules => l.rule_list_height = h,
                    Pane::Editor => l.editor_height = h,
                }
            }
            DragKind::Thumb { target, ratio, max } => {
                let delta = if target.is_vertical() { dy } else { dx };
                let pos = (drag.start_value + delta * ratio).clamp(0.0, max);
                let handle = self.scroll_handle(target);
                let offset = handle.offset();
                handle.set_offset(if target.is_vertical() {
                    point(offset.x, px(-pos))
                } else {
                    point(px(-pos), offset.y)
                });
            }
        }
        cx.notify();
    }

    fn on_root_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.end_drag(cx);
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            self.save_session();
            cx.notify();
        }
    }

    // ---------------------------------------------------------------- 実行・Undo

    pub(crate) fn run_rename(&mut self, cx: &mut Context<Self>) {
        if !self.plan.can_execute() {
            return;
        }
        let (message, is_error) = match execute_plan(&self.plan) {
            Ok(journal) => {
                let count = journal.operations.len();
                self.remap_paths(&journal.operations);
                match journal.save_default() {
                    Ok(()) => {
                        self.can_undo = true;
                        (format!("{count} 件をリネームしました"), false)
                    }
                    Err(err) => {
                        self.can_undo = false;
                        (
                            format!("{count} 件をリネームしました（Undo 情報の保存に失敗: {err}）"),
                            true,
                        )
                    }
                }
            }
            Err(err) => (
                format!("リネームを中止し、変更を元に戻しました: {err}"),
                true,
            ),
        };
        self.excluded.clear();
        self.clear_file_selection();
        self.rescan(cx);
        self.set_status(message, is_error);
    }

    pub(crate) fn run_undo(&mut self, cx: &mut Context<Self>) {
        let Some(journal) = UndoJournal::load_default() else {
            self.can_undo = false;
            self.set_status("元に戻せる操作がありません", true);
            cx.notify();
            return;
        };
        let (message, is_error) = match undo(&journal) {
            Ok(done) => {
                UndoJournal::clear_default();
                self.can_undo = false;
                self.remap_paths(&done.operations);
                (
                    format!("{} 件を元に戻しました", done.operations.len()),
                    false,
                )
            }
            Err(err) => (format!("元に戻せませんでした: {err}"), true),
        };
        self.clear_file_selection();
        self.rescan(cx);
        self.set_status(message, is_error);
    }

    /// リネームで変わったパスに、入力・削除済みリストを追従させる。
    fn remap_paths(&mut self, ops: &[Operation]) {
        for input in &mut self.inputs {
            *input = remap_path(input, ops);
        }
        self.removed = self.removed.iter().map(|p| remap_path(p, ops)).collect();
    }

    pub(crate) fn set_status(&mut self, message: impl Into<String>, is_error: bool) {
        self.status = message.into();
        self.status_is_error = is_error;
    }
}

/// 保存されていたサイズが極端な値でも操作できるようにする。
fn sanitize_layout(mut l: Layout) -> Layout {
    let d = Layout::default();
    let fix = |v: f32, min: f32, default: f32| {
        if v.is_finite() && v >= min {
            v
        } else {
            default
        }
    };
    l.left_width = fix(l.left_width, MIN_LEFT_WIDTH, d.left_width);
    l.preset_height = fix(l.preset_height, MIN_PANE_HEIGHT, d.preset_height);
    l.rule_list_height = fix(l.rule_list_height, MIN_PANE_HEIGHT, d.rule_list_height);
    l.editor_height = fix(l.editor_height, MIN_PANE_HEIGHT, d.editor_height);
    for (w, dw) in l.column_widths.iter_mut().zip(d.column_widths) {
        *w = fix(*w, MIN_COLUMN_WIDTH, dw);
    }
    l
}

/// パス自体、またはその親フォルダがリネームされていれば新しいパスを返す。
fn remap_path(path: &Path, ops: &[Operation]) -> PathBuf {
    let mut path = path.to_path_buf();
    for op in ops {
        if path == op.from {
            path = op.to.clone();
        } else if let Ok(rest) = path.strip_prefix(&op.from) {
            path = op.to.join(rest);
        }
    }
    path
}

impl Render for RenamerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.dark = match self.theme_mode {
            ThemeMode::System => matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
        };
        let t = self.theme();
        let drag_cursor = self.drag.map(|d| match d.kind {
            DragKind::Column(_) | DragKind::LeftPanel => CursorStyle::ResizeColumn,
            DragKind::Pane(_) => CursorStyle::ResizeRow,
            DragKind::Thumb { .. } => CursorStyle::Arrow,
        });

        div()
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                this.add_inputs(paths.paths().to_vec(), cx);
            }))
            .on_mouse_move(cx.listener(Self::on_root_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_root_mouse_up))
            .when_some(drag_cursor, |d, cursor| d.cursor(cursor))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(t.bg))
            .text_color(rgb(t.text))
            .text_sm()
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(self.render_rule_panel(window, cx))
                    .child(self.vertical_splitter(cx))
                    .child(self.render_file_list(window, cx)),
            )
            .child(self.render_status_bar(cx))
    }
}
