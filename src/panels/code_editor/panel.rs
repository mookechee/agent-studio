use std::{path::PathBuf, rc::Rc, str::FromStr};

use autocorrect::ignorer::Ignorer;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    highlighter::{Diagnostic, DiagnosticSeverity, Language},
    input::{Input, InputEvent, InputState, Position, RopeExt, TabSize},
    list::ListItem,
    resizable::{h_resizable, resizable_panel},
    tree::{tree, TreeState},
    v_flex, ActiveTheme, IconName, Sizable, WindowExt,
};
use lsp_types::{CodeActionKind, TextEdit, WorkspaceEdit};

use super::lsp_providers::TextConvertor;
use super::lsp_store::CodeEditorPanelLspStore;
use super::types::build_file_items;

pub struct CodeEditorPanel {
    editor: Entity<InputState>,
    tree_state: Entity<TreeState>,
    go_to_line_state: Entity<InputState>,
    language: Language,
    line_number: bool,
    indent_guides: bool,
    soft_wrap: bool,
    lsp_store: CodeEditorPanelLspStore,
    _subscriptions: Vec<Subscription>,
    _lint_task: Task<()>,
}

impl crate::panels::dock_panel::DockPanel for CodeEditorPanel {
    fn title() -> &'static str {
        "CodeEditor"
    }

    fn description() -> &'static str {
        "A list displays a series of items."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }

    fn paddings() -> Pixels {
        px(0.)
    }
}

impl CodeEditorPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let default_language = Language::from_str("rust");
        let lsp_store = CodeEditorPanelLspStore::new();

        let editor = cx.new(|cx| {
            let mut editor = InputState::new(window, cx)
                .code_editor(default_language.name())
                .line_number(true)
                .indent_guides(true)
                .tab_size(TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                })
                .soft_wrap(false)
                .default_value(include_str!("../../fixtures/test.rs"))
                .placeholder("Enter your code here...");

            let lsp_store = Rc::new(lsp_store.clone());
            editor.lsp.completion_provider = Some(lsp_store.clone());
            editor.lsp.code_action_providers = vec![lsp_store.clone(), Rc::new(TextConvertor)];
            editor.lsp.hover_provider = Some(lsp_store.clone());
            editor.lsp.definition_provider = Some(lsp_store.clone());
            editor.lsp.document_color_provider = Some(lsp_store.clone());

            editor
        });
        let go_to_line_state = cx.new(|cx| InputState::new(window, cx));

        let tree_state = cx.new(|cx| TreeState::new(cx));
        Self::load_files(tree_state.clone(), PathBuf::from("./"), cx);

        let _subscriptions = vec![cx.subscribe(&editor, |this, _, _: &InputEvent, cx| {
            this.lint_document(cx);
        })];

        Self {
            editor,
            tree_state,
            go_to_line_state,
            language: default_language,
            line_number: true,
            indent_guides: true,
            soft_wrap: false,
            lsp_store,
            _subscriptions,
            _lint_task: Task::ready(()),
        }
    }

    fn load_files(state: Entity<TreeState>, path: PathBuf, cx: &mut App) {
        cx.spawn(async move |cx| {
            let ignorer = Ignorer::new(&path.to_string_lossy());
            let items = build_file_items(&ignorer, &path, &path);
            _ = state.update(cx, |state, cx| {
                state.set_items(items, cx);
            });
        })
        .detach();
    }

    fn go_to_line(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        let input_state = self.go_to_line_state.clone();

        window.open_dialog(cx, move |dialog, window, cx| {
            input_state.update(cx, |state, cx| {
                let cursor_pos = editor.read(cx).cursor_position();
                state.set_placeholder(
                    format!("{}:{}", cursor_pos.line, cursor_pos.character),
                    window,
                    cx,
                );
                state.focus(window, cx);
            });

            dialog
                .title("Go to line")
                .child(Input::new(&input_state))
                .confirm()
                .on_ok({
                    let editor = editor.clone();
                    let input_state = input_state.clone();
                    move |_, window, cx| {
                        let query = input_state.read(cx).value();
                        let mut parts = query
                            .split(':')
                            .map(|s| s.trim().parse::<usize>().ok())
                            .collect::<Vec<_>>()
                            .into_iter();
                        let Some(line) = parts.next().and_then(|l| l) else {
                            return false;
                        };
                        let column = parts.next().and_then(|c| c).unwrap_or(1);
                        let position = gpui_component::input::Position::new(
                            line.saturating_sub(1) as u32,
                            column.saturating_sub(1) as u32,
                        );

                        editor.update(cx, |state, cx| {
                            state.set_cursor_position(position, window, cx);
                        });

                        true
                    }
                })
        });
    }

    fn lint_document(&mut self, cx: &mut Context<Self>) {
        let language = self.language.name().to_string();
        let lsp_store = self.lsp_store.clone();
        let text = self.editor.read(cx).text().clone();

        self._lint_task = cx.background_spawn(async move {
            let value = text.to_string();
            let result = autocorrect::lint_for(value.as_str(), &language);

            let mut code_actions = vec![];
            let mut diagnostics = vec![];

            for item in result.lines.iter() {
                let severity = match item.severity {
                    autocorrect::Severity::Error => DiagnosticSeverity::Warning,
                    autocorrect::Severity::Warning => DiagnosticSeverity::Hint,
                    autocorrect::Severity::Pass => DiagnosticSeverity::Info,
                };

                let line = item.line.saturating_sub(1); // Convert to 0-based index
                let col = item.col.saturating_sub(1); // Convert to 0-based index

                let start = Position::new(line as u32, col as u32);
                let end = Position::new(line as u32, (col + item.old.chars().count()) as u32);
                let message = format!("AutoCorrect: {}", item.new);
                diagnostics.push(Diagnostic::new(start..end, message).with_severity(severity));

                let range = text.position_to_offset(&start)..text.position_to_offset(&end);

                let text_edit = TextEdit {
                    range: lsp_types::Range { start, end },
                    new_text: item.new.clone(),
                    ..Default::default()
                };

                let edit = WorkspaceEdit {
                    changes: Some(
                        std::iter::once((
                            lsp_types::Uri::from_str("file://CodeEditorPanel").unwrap(),
                            vec![text_edit],
                        ))
                        .collect(),
                    ),
                    ..Default::default()
                };

                code_actions.push((
                    range,
                    lsp_types::CodeAction {
                        title: format!("Change to '{}'", item.new),
                        kind: Some(CodeActionKind::QUICKFIX),
                        edit: Some(edit),
                        ..Default::default()
                    },
                ));
            }

            lsp_store.update_code_actions(code_actions.clone());
            lsp_store.update_diagnostics(diagnostics.clone());
        });
    }

    fn open_file(
        view: Entity<Self>,
        path: PathBuf,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<()> {
        let language = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default();
        let language = Language::from_str(&language);
        let content = std::fs::read_to_string(&path)?;

        window
            .spawn(cx, async move |window| {
                _ = view.update_in(window, |this, window, cx| {
                    _ = this.editor.update(cx, |this, cx| {
                        this.set_highlighter(language.name(), cx);
                        this.set_value(content, window, cx);
                    });

                    this.language = language;
                    cx.notify();
                });
            })
            .detach();

        Ok(())
    }

    fn render_file_tree(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        tree(
            &self.tree_state,
            move |ix, entry, _selected, _window, cx| {
                view.update(cx, |_, cx| {
                    let item = entry.item();
                    let icon = if !entry.is_folder() {
                        IconName::File
                    } else if entry.is_expanded() {
                        IconName::FolderOpen
                    } else {
                        IconName::Folder
                    };

                    ListItem::new(ix)
                        .w_full()
                        .rounded(cx.theme().radius)
                        .py_0p5()
                        .px_2()
                        .pl(px(16.) * entry.depth() + px(8.))
                        .child(h_flex().gap_2().child(icon).child(item.label.clone()))
                        .on_click(cx.listener({
                            let item = item.clone();
                            move |_, _, _window, cx| {
                                if item.is_folder() {
                                    return;
                                }

                                Self::open_file(
                                    cx.entity(),
                                    PathBuf::from(item.id.as_str()),
                                    _window,
                                    cx,
                                )
                                .ok();

                                cx.notify();
                            }
                        }))
                })
            },
        )
        .text_sm()
        .p_1()
        .bg(cx.theme().sidebar)
        .text_color(cx.theme().sidebar_foreground)
        .h_full()
    }

    fn render_line_number_button(
        &self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        Button::new("line-number")
            .when(self.line_number, |this| this.icon(IconName::Check))
            .label("Line Number")
            .ghost()
            .xsmall()
            .on_click(cx.listener(|this, _, window, cx| {
                this.line_number = !this.line_number;
                this.editor.update(cx, |state, cx| {
                    state.set_line_number(this.line_number, window, cx);
                });
                cx.notify();
            }))
    }

    fn render_soft_wrap_button(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        Button::new("soft-wrap")
            .ghost()
            .xsmall()
            .when(self.soft_wrap, |this| this.icon(IconName::Check))
            .label("Soft Wrap")
            .on_click(cx.listener(|this, _, window, cx| {
                this.soft_wrap = !this.soft_wrap;
                this.editor.update(cx, |state, cx| {
                    state.set_soft_wrap(this.soft_wrap, window, cx);
                });
                cx.notify();
            }))
    }

    fn render_indent_guides_button(
        &self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        Button::new("indent-guides")
            .ghost()
            .xsmall()
            .when(self.indent_guides, |this| this.icon(IconName::Check))
            .label("Indent Guides")
            .on_click(cx.listener(|this, _, window, cx| {
                this.indent_guides = !this.indent_guides;
                this.editor.update(cx, |state, cx| {
                    state.set_indent_guides(this.indent_guides, window, cx);
                });
                cx.notify();
            }))
    }

    fn render_go_to_line_button(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let position = self.editor.read(cx).cursor_position();
        let cursor = self.editor.read(cx).cursor();

        Button::new("line-column")
            .ghost()
            .xsmall()
            .label(format!(
                "{}:{} ({} byte)",
                position.line + 1,
                position.character + 1,
                cursor
            ))
            .on_click(cx.listener(Self::go_to_line))
    }
}

impl Render for CodeEditorPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Update diagnostics
        if self.lsp_store.is_dirty() {
            let diagnostics = self.lsp_store.diagnostics();
            self.editor.update(cx, |state, cx| {
                state.diagnostics_mut().map(|set| {
                    set.clear();
                    set.extend(diagnostics);
                });
                cx.notify();
            });
        }

        v_flex().id("app").size_full().child(
            v_flex()
                .id("source")
                .w_full()
                .flex_1()
                .child(
                    h_resizable("editor-container")
                        .child(
                            resizable_panel()
                                .size(px(240.))
                                .child(self.render_file_tree(window, cx)),
                        )
                        .child(
                            Input::new(&self.editor)
                                .bordered(false)
                                .p_0()
                                .h_full()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_size(cx.theme().mono_font_size)
                                .focus_bordered(false)
                                .into_any_element(),
                        ),
                )
                .child(
                    h_flex()
                        .justify_between()
                        .text_sm()
                        .bg(cx.theme().background)
                        .py_1p5()
                        .px_4()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            h_flex()
                                .gap_3()
                                .child(self.render_line_number_button(window, cx))
                                .child(self.render_soft_wrap_button(window, cx))
                                .child(self.render_indent_guides_button(window, cx)),
                        )
                        .child(self.render_go_to_line_button(window, cx)),
                ),
        )
    }
}
