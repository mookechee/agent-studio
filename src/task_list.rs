use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Duration};

use fake::Fake;
use gpui::{
    App, AppContext, ClickEvent, Context, ElementId, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, MouseButton, ParentElement, Render, RenderOnce, SharedString, Styled, Subscription, Task, Timer, Window, actions, div, px, prelude::FluentBuilder
};

use gpui_component::{
    ActiveTheme, Icon, IconName, IndexPath, Selectable, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    spinner::Spinner,
    v_flex,
};

actions!(list_story, [SelectedAgentTask]);

/// Task status enumeration
#[derive(Clone, Default, Debug)]
enum TaskStatus {
    /// Task is pending
    #[default]
    Pending,
    /// Task is currently running
    InProgress,
    /// Task completed successfully
    Completed,
    /// Task failed to complete
    Failed,
}

#[derive(Clone, Default)]
struct AgentTask {
    name: SharedString,
    task_type: SharedString,
    add_new_code_lines: i16,
    delete_code_lines: i16,
    status: TaskStatus,

    change_timestamp: i16,
    change_timestamp_str: SharedString,
    add_new_code_lines_str: SharedString,
    delete_code_lines_str: SharedString,
    // description: String,
}

impl AgentTask {
    fn prepare(mut self) -> Self {
        // self.change_timestamp = (self.add_new_code_lines - self.delete_code_lines) / self.delete_code_lines;
        // self.change_timestamp_str = format!("{}", self.change_timestamp).into();
        self.add_new_code_lines_str = format!("+{}", self.add_new_code_lines).into();
        self.delete_code_lines_str = format!("-{}", self.delete_code_lines).into();
        self
    }
}

#[derive(IntoElement)]
struct TaskListItem {
    base: ListItem,
    agent_task: Rc<AgentTask>,
    selected: bool,
}

impl TaskListItem {
    pub fn new(id: impl Into<ElementId>, agent_task: Rc<AgentTask>, selected: bool) -> Self {
        TaskListItem {
            agent_task,
            base: ListItem::new(id).selected(selected),
            selected,
        }
    }
}

impl Selectable for TaskListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for TaskListItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_color = if self.selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        let muted_color = cx.theme().muted_foreground;
        let add_color = cx.theme().green;
        let delete_color = cx.theme().red;

        // Show metadata only when not selected
        let show_metadata = !self.selected;

        // Check if task is in progress to use Spinner
        let is_in_progress = matches!(self.agent_task.status, TaskStatus::InProgress);

        self.base
            .px_3()
            .py_2()
            .overflow_x_hidden()
            .rounded(cx.theme().radius)
            .child(
                h_flex()
                    .items_start()  // Top align instead of center
                    .gap_3()
                    .mt(px(2.))
                    .child(
                        div()
                            .mt(px(2.))
                            .map(|this| {
                                if is_in_progress {
                                    // Use Spinner for InProgress status
                                    this.child(
                                        Spinner::new()
                                            .with_size(px(14.))
                                            .color(cx.theme().accent)
                                    )
                                } else {
                                    // Use Icon for other statuses
                                    let (icon_name, icon_color) = match self.agent_task.status {
                                        TaskStatus::Pending => (IconName::File, muted_color),
                                        TaskStatus::Completed => (IconName::CircleCheck, cx.theme().green),
                                        TaskStatus::Failed => (IconName::CircleX, cx.theme().red),
                                        _ => (IconName::File, muted_color),
                                    };
                                    this.child(
                                        Icon::new(icon_name)
                                            .text_color(icon_color)
                                            .size(px(14.))
                                    )
                                }
                            })
                    )
                    .child(
                        // Vertical layout for title and subtitle
                        v_flex()
                            .gap_0p5()
                            .flex_1()
                            .overflow_x_hidden()
                            .child(
                                // Title - reduced font size
                                div()
                                    .text_size(px(13.))
                                    .text_color(text_color)
                                    .whitespace_nowrap()
                                    .child(self.agent_task.name.clone())
                            )
                            .child(
                                // Subtitle with metadata - conditionally shown
                                h_flex()
                                    .gap_1()
                                    .text_size(px(11.))
                                    .text_color(muted_color)
                                    .child("2 Files ")
                                    .child(
                                        div()
                                            .text_color(add_color)
                                            .child(self.agent_task.add_new_code_lines_str.clone())
                                    )
                                    .child(
                                        div()
                                            .text_color(delete_color)
                                            .child(self.agent_task.delete_code_lines_str.clone())
                                    )
                                    .child(" · ")
                                    .child(self.agent_task.task_type.clone())
                            )

                    )
            )
    }
}

struct TaskListDelegate {
    industries: Vec<SharedString>,
    _agent_tasks: Vec<Rc<AgentTask>>,
    matched_agent_tasks: Vec<Vec<Rc<AgentTask>>>,
    selected_index: Option<IndexPath>,
    confirmed_index: Option<IndexPath>,
    query: SharedString,
    loading: bool,
    eof: bool,
    lazy_load: bool,
    // Track which sections are collapsed (using RefCell for interior mutability)
    collapsed_sections: Rc<RefCell<HashSet<usize>>>,
}

impl TaskListDelegate {
    fn toggle_section_collapsed(&self, section: usize) {
        let mut collapsed = self.collapsed_sections.borrow_mut();
        if collapsed.contains(&section) {
            collapsed.remove(&section);
        } else {
            collapsed.insert(section);
        }
    }

    fn is_section_collapsed(&self, section: usize) -> bool {
        self.collapsed_sections.borrow().contains(&section)
    }

    fn prepare(&mut self, query: impl Into<SharedString>) {
        self.query = query.into();
        let agent_tasks: Vec<Rc<AgentTask>> = self
            ._agent_tasks
            .iter()
            .filter(|agent_task| {
                agent_task
                    .name
                    .to_lowercase()
                    .contains(&self.query.to_lowercase())
            })
            .cloned()
            .collect();
        for agent_task in agent_tasks.into_iter() {
            if let Some(ix) = self.industries.iter().position(|s| s == &agent_task.task_type) {
                self.matched_agent_tasks[ix].push(agent_task);
            } else {
                self.industries.push(agent_task.task_type.clone());
                self.matched_agent_tasks.push(vec![agent_task]);
            }
        }
    }

    fn extend_more(&mut self, len: usize) {
        self._agent_tasks
            .extend((0..len).map(|_| Rc::new(random_agent_task())));
        self.prepare(self.query.clone());
    }

    fn selected_agent_task(&self) -> Option<Rc<AgentTask>> {
        let Some(ix) = self.selected_index else {
            return None;
        };

        self.matched_agent_tasks
            .get(ix.section)
            .and_then(|c| c.get(ix.row))
            .cloned()
    }
}

impl ListDelegate for TaskListDelegate {
    type Item = TaskListItem;

    fn sections_count(&self, _: &App) -> usize {
        self.industries.len()
    }

    fn items_count(&self, section: usize, _: &App) -> usize {
        // Return 0 items if the section is collapsed
        if self.is_section_collapsed(section) {
            0
        } else {
            self.matched_agent_tasks[section].len()
        }
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.prepare(query.to_owned());
        Task::ready(())
    }

    fn confirm(&mut self, secondary: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        println!("Confirmed with secondary: {}", secondary);
        window.dispatch_action(Box::new(SelectedAgentTask), cx);
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn render_section_header(
        &self,
        section: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement> {
        let Some(task_type) = self.industries.get(section) else {
            return None;
        };

        let is_collapsed = self.is_section_collapsed(section);
        let collapsed_sections = self.collapsed_sections.clone();

        // Use ChevronRight when collapsed, ChevronDown when expanded
        let chevron_icon = if is_collapsed {
            IconName::ChevronRight
        } else {
            IconName::ChevronDown
        };

        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .pb_1()
                .px_2()
                .gap_2()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .cursor_default()
                .rounded(cx.theme().radius)
                .hover(|style| {
                    style.bg(cx.theme().secondary)
                })
                .on_mouse_down(MouseButton::Left, move |_, window, _cx| {
                    // Toggle the collapsed state
                    let mut collapsed = collapsed_sections.borrow_mut();
                    if collapsed.contains(&section) {
                        collapsed.remove(&section);
                    } else {
                        collapsed.insert(section);
                    }
                    // Request a refresh to update the UI
                    window.refresh();
                })
                .child(Icon::new(chevron_icon).size(px(14.)))
                .child(Icon::new(IconName::Folder))
                .child(task_type.clone()),
        )
    }

    fn render_section_footer(
        &self,
        section: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement> {
        let Some(_) = self.industries.get(section) else {
            return None;
        };

        Some(
            div()
                .pt_1()
                .pb_5()
                .px_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "Total {} items in section.",
                    self.matched_agent_tasks[section].len()
                )),
        )
    }

    fn render_item(&self, ix: IndexPath, _: &mut Window, _: &mut App) -> Option<Self::Item> {
        let selected = Some(ix) == self.selected_index || Some(ix) == self.confirmed_index;
        if let Some(agent_task) = self.matched_agent_tasks[ix.section].get(ix.row) {
            return Some(TaskListItem::new(ix, agent_task.clone(), selected));
        }

        None
    }

    fn loading(&self, _: &App) -> bool {
        self.loading
    }

    fn is_eof(&self, _: &App) -> bool {
        return !self.loading && !self.eof;
    }

    fn load_more_threshold(&self) -> usize {
        150
    }

    fn load_more(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        if !self.lazy_load {
            return;
        }

        cx.spawn_in(window, async move |view, window| {
            // Simulate network request, delay 1s to load data.
            Timer::after(Duration::from_secs(1)).await;

            _ = view.update_in(window, move |view, window, cx| {
                let query = view.delegate().query.clone();
                view.delegate_mut().extend_more(200);
                _ = view.delegate_mut().perform_search(&query, window, cx);
                view.delegate_mut().eof = view.delegate()._agent_tasks.len() >= 6000;
            });
        })
        .detach();
    }
}

pub struct ListStory {
    focus_handle: FocusHandle,
    task_list: Entity<ListState<TaskListDelegate>>,
    selected_agent_task: Option<Rc<AgentTask>>,
    selectable: bool,
    searchable: bool,
    _subscriptions: Vec<Subscription>,
}

impl super::Story for ListStory {
    fn title() -> &'static str {
        "List"
    }

    fn description() -> &'static str {
        "A list displays a series of items."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl ListStory {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut delegate = TaskListDelegate {
            industries: vec![],
            matched_agent_tasks: vec![vec![]],
            _agent_tasks: vec![],
            selected_index: Some(IndexPath::default()),
            confirmed_index: None,
            query: "".into(),
            loading: false,
            eof: false,
            lazy_load: false,
            collapsed_sections: Rc::new(RefCell::new(HashSet::new())),
        };
        delegate.extend_more(100);

        let task_list = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));

        let _subscriptions =
            vec![
                cx.subscribe(&task_list, |_, _, ev: &ListEvent, _| match ev {
                    ListEvent::Select(ix) => {
                        println!("List Selected: {:?}", ix);
                    }
                    ListEvent::Confirm(ix) => {
                        println!("List Confirmed: {:?}", ix);
                    }
                    ListEvent::Cancel => {
                        println!("List Cancelled");
                    }
                }),
            ];

        // Spawn a background to random refresh the list
        cx.spawn(async move |this, cx| {
            this.update(cx, |this, cx| {
                this.task_list.update(cx, |picker, _| {
                    picker
                        .delegate_mut()
                        ._agent_tasks
                        .iter_mut()
                        .for_each(|agent_task| {
                            let mut new_agent_task = random_agent_task();
                            new_agent_task.name = agent_task.name.clone();
                            new_agent_task.task_type = agent_task.task_type.clone();
                            *agent_task = Rc::new(new_agent_task);
                        });
                    picker.delegate_mut().prepare("");
                });
                cx.notify();
            })
            .ok();
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            searchable: true,
            selectable: true,
            task_list,
            selected_agent_task: None,
            _subscriptions,
        }
    }

    fn selected_agent_task(&mut self, _: &SelectedAgentTask, _: &mut Window, cx: &mut Context<Self>) {
        let picker = self.task_list.read(cx);
        if let Some(agent_task) = picker.delegate().selected_agent_task() {
            self.selected_agent_task = Some(agent_task);
        }
    }

    fn toggle_selectable(&mut self, selectable: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.selectable = selectable;
        self.task_list.update(cx, |list, cx| {
            list.set_selectable(self.selectable, cx);
        })
    }

    fn toggle_searchable(&mut self, searchable: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.searchable = searchable;
        self.task_list.update(cx, |list, cx| {
            list.set_searchable(self.searchable, cx);
        })
    }

    fn on_click(ev: &ClickEvent, _: &mut Window, _: &mut App) {
        println!("Button clicked {:?}", ev);
    }
}

fn random_agent_task() -> AgentTask {
    let add_new_code_lines = (0..999).fake::<i16>();
    let delete_code_lines = (0..999).fake::<i16>();

    // Randomly generate task status
    let status = match (0..4).fake::<u8>() {
        0 => TaskStatus::Pending,
        1 => TaskStatus::InProgress,
        2 => TaskStatus::Completed,
        _ => TaskStatus::Failed,
    };

    AgentTask {
        name: fake::faker::filesystem::en::FileName()
            .fake::<String>()
            .into(),
        task_type: fake::faker::filesystem::en::MimeType().fake::<String>().into(),
        add_new_code_lines,
        delete_code_lines,
        status,
        ..Default::default()
    }
    .prepare()
}

impl Focusable for ListStory {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ListStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // let lazy_load = self.task_list.read(cx).delegate().lazy_load;

        v_flex()
            .child(
                Button::new("btn-new-task")
                    .label("New Task")
                    .primary()
                    .icon(Icon::new(IconName::Plus))
                    // .disabled(loading)
                    // .loading(loading)
                    .on_click(Self::on_click),
            )
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::selected_agent_task))
            .size_full()
            .gap_4()
            .child(
                List::new(&self.task_list)
                    .p(px(8.))
                    .flex_1()
                    .w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius),
            )
    }
}
