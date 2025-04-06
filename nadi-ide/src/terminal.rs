use crate::editor::my_hl;
use crate::help::md_style;
use crate::icons;
use crate::network::{NetworkData, NetworkTable};
use iced::widget::{
    button, center, column, combo_box, container, horizontal_rule, horizontal_space, markdown,
    progress_bar, row, scrollable, text, text_editor, text_input, toggler,
};
use iced::{Element, Fill, Font, Length, Task, Theme};
use nadi_core::string_template::Template;
use nadi_core::tasks::{Task as NadiTask, TaskContext};
use std::io::Read;
use std::sync::Arc;

pub static NETWORK_HELP: &str = include_str!("../markdown/network.md");

pub struct Terminal {
    pub light_theme: bool,
    running_msg: Option<String>,
    history_str: Vec<String>,
    history: combo_box::State<String>,
    command: String,
    status: String,
    content: text_editor::Content,
    pub task_ctx: TaskContext,
    progress: f32,
    network: NetworkData,
    network_sidebar: bool,
    network_help: Vec<markdown::Item>,
    label_template: String,
    embedded: bool,
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            light_theme: false,
            running_msg: None,
            history_str: vec![],
            history: combo_box::State::<String>::default(),
            command: String::new(),
            status: String::new(),
            content: text_editor::Content::default(),
            task_ctx: TaskContext::new(None),
            progress: 0.0,
            network: NetworkData::default(),
            network_sidebar: false,
            network_help: markdown::parse(NETWORK_HELP).collect(),
            label_template: String::new(),
            embedded: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    ThemeChange(bool),
    EditorAction(text_editor::Action),
    SaveHistory,
    Run(String),
    ExecCommand,
    RunTasks(String),
    TemplChange(String),
    TemplSubmit,
    TaskChain(usize, Vec<NadiTask>),
    CommandChange(String),
    History(String),
    GotoTop,
    GotoBottom,
    GoUp,
    GoDown,
    ToggleNetSidebar,
    LinkClicked(markdown::Url),
    // handled in main
    NodeClicked(Option<String>),
}

impl Terminal {
    pub fn embed(mut self) -> Self {
        self.embedded = true;
        self
    }

    pub fn append_history(&mut self, entry: String) {
        self.history_str.push(entry);
        self.history = combo_box::State::new(self.history_str.clone());
    }

    // Can't do async because the TaskContext is not thread
    // safe. Might have to find a way to run it using channels
    fn execute_task(&mut self, task: NadiTask) -> (String, Result<Option<String>, String>) {
        // temp solution, make NadiFunctions take a std::io::Write or
        // other trait object that can either print to stdout, or take the
        // result to show somewhere else (like here)
        let mut buf = gag::BufferRedirect::stdout().unwrap();
        let mut output = String::new();
        let mut results = String::new();
        let res = self.task_ctx.execute(task);
        // print the stdout output to the terminal
        buf.read_to_string(&mut output).unwrap();
        output.push('\n');
        match res {
            Ok(Some(p)) => {
                results.push_str(&p);
                results.push('\n');
            }
            Err(e) => return (output, Err(e.to_string())),
            _ => (),
        }
        (output, Ok(Some(results)))
    }

    fn append_term(&mut self, text: &str) {
        self.content
            .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
        self.content
            .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new(format!("{}\n", text.trim())),
            )));
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        self.status.clear();
        match message {
            Message::ThemeChange(theme) => {
                self.light_theme = theme;
            }
            Message::ToggleNetSidebar => {
                self.network_sidebar = !self.network_sidebar;
            }
            Message::LinkClicked(url) => {
                _ = webbrowser::open(url.as_ref());
            }
            Message::EditorAction(action) => {
                // We don't allow the editor to be edited by users at all
                if action.is_edit() {
                    self.status.push_str("Edit not permitted");
                } else {
                    self.content.perform(action);
                }
            }
            Message::CommandChange(cmd) => {
                self.command = cmd;
            }
            Message::TaskChain(done, mut tasks) => {
                let task = if let Some(t) = tasks.pop() {
                    t
                } else {
                    self.progress = 100.0;
                    self.running_msg = None;
                    return Task::none();
                };
                let (out, res) = self.execute_task(task);
                self.append_term(&out);
                match res {
                    Ok(Some(s)) => self.append_term(&s),
                    Err(s) => self.append_term(&s),
                    _ => (),
                };
                self.network.update(
                    &self.task_ctx.network,
                    if self.label_template.is_empty() {
                        None
                    } else {
                        Template::parse_template(&self.label_template).ok()
                    },
                );
                self.progress = (done + 1) as f32 * 100.0 / (done + 1 + tasks.len()) as f32;
                self.running_msg = Some(format!("Executing Tasks: {:.2}%", self.progress));
                return Task::perform(async { tasks }, move |t| Message::TaskChain(done + 1, t));
            }
            Message::RunTasks(tasks) => {
                self.append_term(&tasks);
                self.progress = 0.0;
                let tasks_vec = match nadi_core::parser::tokenizer::get_tokens(&tasks) {
                    Ok(tk) => match nadi_core::parser::tasks::parse(tk) {
                        Ok(t) => t,
                        Err(e) => {
                            self.running_msg = None;
                            self.status = e.to_string();
                            return Task::none();
                        }
                    },
                    Err(e) => {
                        self.running_msg = None;
                        self.status = e.to_string();
                        return Task::none();
                    }
                };
                self.append_history(tasks);
                self.running_msg = Some(format!("Executing Tasks: {:.2}%", self.progress));
                return Task::perform(async { tasks_vec.into_iter().rev().collect() }, move |t| {
                    Message::TaskChain(0, t)
                });
            }
            Message::ExecCommand => {
                let task = self.command.clone();
                self.command.clear();
                match task.split_once(" ") {
                    Some(("attr", args)) => {
                        let a = args.to_string();
                        return Task::perform(async { Some(a) }, Message::NodeClicked);
                    }
                    // Some(("help", args)) => ,
                    None if task == "attr" => {
                        return Task::perform(async { None }, Message::NodeClicked);
                    }
                    _ => (),
                };
                self.running_msg = Some("Executing Command".to_string());
                return Task::perform(async { task }, Message::RunTasks);
            }
            Message::TemplChange(templ) => {
                self.label_template = templ;
            }
            Message::TemplSubmit => {
                self.network.update(
                    &self.task_ctx.network,
                    if self.label_template.is_empty() {
                        None
                    } else {
                        Template::parse_template(&self.label_template).ok()
                    },
                );
            }
            Message::GotoTop => {
                self.content.perform(text_editor::Action::Move(
                    text_editor::Motion::DocumentStart,
                ));
            }
            Message::GotoBottom => {
                self.content
                    .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
            }
            Message::GoUp => {
                self.content
                    .perform(text_editor::Action::Move(text_editor::Motion::PageUp));
            }
            Message::GoDown => {
                self.content
                    .perform(text_editor::Action::Move(text_editor::Motion::PageDown));
            }
            Message::History(hist) => {
                self.command = hist;
            }
            _ => (),
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut controls = row![
            icons::action(icons::top_icon(), "Goto Top", Some(Message::GotoTop)),
            icons::action(icons::up_icon(), "Go Up", Some(Message::GoUp)),
            icons::action(icons::down_icon(), "Go Down", Some(Message::GoDown)),
            icons::action(
                icons::bottom_icon(),
                "Goto Bottom",
                Some(Message::GotoBottom)
            ),
            horizontal_space(),
            combo_box(&self.history, "Search History", None, Message::History)
        ];
        if !self.embedded {
            controls = controls.push(toggler(self.light_theme).on_toggle(Message::ThemeChange));
        }
        let entry = row![
            text_input(
                self.running_msg.as_deref().unwrap_or("Command"),
                &self.command
            )
            .on_input_maybe(
                self.running_msg
                    .as_ref()
                    .is_none()
                    .then_some(Message::CommandChange)
            )
            .on_submit(Message::ExecCommand)
            .font(Font::MONOSPACE),
        ];
        column![
            controls.spacing(10).padding(10),
            text_editor(&self.content)
                .height(Fill)
                .font(Font::MONOSPACE)
                .on_action(Message::EditorAction)
                .highlight_with::<my_hl::NadiHighlighter>(
                    my_hl::NadiFileType::Terminal,
                    my_hl::Highlight::to_format
                ),
            text(&self.status),
            entry,
            progress_bar(0.0..=100.0, self.progress)
                .height(4.0)
                .style(progress_bar::success)
        ]
        .padding(10)
        .into()
    }

    pub fn view_network(&self) -> Element<'_, Message> {
        let mut sidebar = row![
            button(center(if self.network_sidebar {
                icons::right_icon()
            } else {
                icons::left_icon()
            }))
            .on_press(Message::ToggleNetSidebar)
            .height(Length::Fill)
            .style(button::secondary)
            .width(25)
        ];
        if self.network_sidebar {
            sidebar = sidebar.push(
                column![
                    text_input("Label Template", &self.label_template)
                        .on_input(Message::TemplChange)
                        .on_submit(Message::TemplSubmit),
                    horizontal_rule(5.0),
                    scrollable(
                        markdown::view(
                            &self.network_help,
                            markdown::Settings::default(),
                            md_style(self.light_theme),
                        )
                        .map(Message::LinkClicked)
                    ),
                ]
                .spacing(10.0)
                .padding(10.0),
            );
        }
        row![
            scrollable(
                container(NetworkTable::new(&self.network).on_press(Message::NodeClicked))
                    .padding(10.0)
            )
            .width(Fill)
            .height(Fill),
            sidebar
        ]
        .spacing(10.0)
        .into()
    }

    pub fn theme(&self) -> Theme {
        if self.light_theme {
            Theme::Light
        } else {
            Theme::Dark
        }
    }
}
