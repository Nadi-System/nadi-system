use crate::editor::{Completion, my_hl};
use crate::help::md_style;
use crate::icons;
use crate::network::{NetworkData, NetworkDataView, NetworkTable, NetworkViewType};
use iced::time::{self, Duration};
use iced::widget::{
    button, center, column, combo_box, container, markdown, mouse_area, pick_list, progress_bar,
    row, scrollable, slider, space::horizontal, text, text_editor, text_input, toggler,
};
use iced::{Element, Fill, Font, Length, Subscription, Task, Theme};
use nadi_core::abi_stable::std_types::RString;
use nadi_core::attrs::{AttrMap, HasAttributes};
use nadi_core::parser::highlight::NadiFileType;
use nadi_core::tasks::{Task as NadiTask, TaskContext, TaskMessage};
use std::io::Read;
use std::ops::Not;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

pub static NETWORK_HELP: &str = include_str!("../markdown/network.md");

/// Message sent by the task context over the channel
enum TaskCtxMessage {
    Network(NetworkData),
    Attribute(String, AttrMap),
    Result(String, Result<Option<String>, String>), // User TaskResult later
    Update(TaskMessage),
    Completions(Vec<Completion>),
    Clear,
    Waiting,
}

/// Messages that can be sent to the task context over the channel
enum TaskCtxRequest {
    NetworkData,
    Run(Box<NadiTask>),
    NodeAttr(String),
    Completions(String),
    NetworkAttr,
    NetworkTy(NetworkViewType),
}

/// Spawns a thread with task context, it reloads the plugins as it is not thread safe to share them.
fn spawn_task_context() -> (Sender<TaskCtxRequest>, Receiver<TaskCtxMessage>) {
    // for receiving the results this function sends
    let (send, recv_outer) = channel();
    // for sending Tasks that this function receives
    let (send_outer, recv) = channel();
    // for communicating with TaskContext
    let (send_inner, recv_inner) = channel();

    // this forwards the messages from task context to the receiver
    let send2 = send.clone();
    thread::spawn(move || {
        for msg in recv_inner {
            let _ = send2.send(TaskCtxMessage::Update(msg));
        }
    });

    thread::spawn(move || {
        let mut task_ctx = TaskContext::new(None, send_inner);
        let mut locals = AttrMap::new();
        let mut ty = NetworkViewType::default();
        loop {
            while let Ok(req) = recv.try_recv() {
                match req {
                    TaskCtxRequest::NetworkData => {
                        let _ = send.send(TaskCtxMessage::Network(NetworkData::new(
                            &task_ctx.network,
                            &ty,
                        )));
                    }
                    TaskCtxRequest::Run(task) => {
                        let mutates = task.can_mutate();
                        // temp solution, make NadiFunctions take a
                        // std::io::Write or other trait object that
                        // can either print to stdout, or take the
                        // result to show somewhere else (like here)
                        let mut buf = gag::BufferRedirect::stdout().unwrap();
                        let mut output = String::new();
                        if matches!(*task, NadiTask::Clear) {
                            task_ctx.clear();
                            let _ = send.send(TaskCtxMessage::Clear);
                        }
                        let res = task_ctx
                            .execute(*task, &mut locals)
                            .map_err(|e| e.to_string());
                        // print the stdout output to the terminal
                        buf.read_to_string(&mut output).unwrap();
                        output.push('\n');
                        let _ = send.send(TaskCtxMessage::Result(output, res));
                        if mutates {
                            // only send this if there might have been new values
                            let _ = send.send(TaskCtxMessage::Network(NetworkData::new(
                                &task_ctx.network,
                                &ty,
                            )));
                        }
                    }
                    TaskCtxRequest::NodeAttr(name) => {
                        if let Some(node) = task_ctx.network.node_by_name(&name) {
                            let am = node.lock().attr_map().clone();
                            let _ = send.send(TaskCtxMessage::Attribute(name, am));
                        }
                    }
                    TaskCtxRequest::NetworkAttr => {
                        let am = task_ctx.network.attr_map().clone();
                        let _ = send.send(TaskCtxMessage::Attribute("Network".to_string(), am));
                    }
                    TaskCtxRequest::NetworkTy(new) => {
                        ty = new;
                        let _ = send.send(TaskCtxMessage::Network(NetworkData::new(
                            &task_ctx.network,
                            &ty,
                        )));
                    }
                    TaskCtxRequest::Completions(compl) => {
                        let _ = send.send(TaskCtxMessage::Completions(task_completion(
                            &task_ctx, compl,
                        )));
                    }
                }
            }
            let _ = send.send(TaskCtxMessage::Waiting);
            thread::sleep(Duration::from_millis(100));
        }
    });
    (send_outer, recv_outer)
}

pub struct Terminal {
    pub light_theme: bool,
    running_msg: Option<String>,
    history_str: Vec<String>,
    history: combo_box::State<String>,
    residue: String,
    command: String,
    status: String,
    content: text_editor::Content,
    last_line: usize,
    sender: Sender<TaskCtxRequest>,
    receiver: Receiver<TaskCtxMessage>,
    progress: (String, f32),
    network_view: NetworkDataView,
    network_ty: NetworkViewType,
    network_sidebar: bool,
    network_help: Vec<markdown::Item>,
    embedded: bool,
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    ThemeChange(bool),
    EditorAction(text_editor::Action),
    SaveHistory,
    Run(String),
    ClearContent,
    ClearCommand,
    ExecCommand,
    RunTasks(String),
    TaskChain(usize, Vec<NadiTask>),
    CommandChange(String),
    History(String),
    GotoTop,
    GotoBottom,
    GoUp,
    GoDown,
    ToggleNetSidebar,
    LinkClicked(String),
    Tick,
    NodeClicked(Option<String>),
    ScaleChanged(f32),
    GetCompletions(String),
    NetworkTyChange(NetworkViewType),
    // handled in main
    AttrFound((String, AttrMap)),
    ComplResult(Vec<Completion>),
    OpenImage(String),
}

impl Terminal {
    pub fn new() -> Self {
        let (sender, receiver) = spawn_task_context();
        Self {
            light_theme: false,
            running_msg: None,
            history_str: vec![],
            history: combo_box::State::<String>::default(),
            residue: String::new(),
            command: String::new(),
            status: String::new(),
            content: text_editor::Content::default(),
            last_line: 0,
            sender,
            receiver,
            progress: (String::new(), 0.0),
            network_view: NetworkDataView::default(),
            network_ty: NetworkViewType::Flat,
            network_sidebar: false,
            network_help: markdown::parse(NETWORK_HELP).collect(),
            embedded: false,
        }
    }
    pub fn embed(mut self) -> Self {
        self.embedded = true;
        self
    }

    pub fn append_history(&mut self, entry: String) {
        self.history_str.push(entry);
        self.history = combo_box::State::new(self.history_str.clone());
    }

    fn append_term(&mut self, text: &str, prompt: bool, append: bool) {
        if text.is_empty() {
            return;
        }
        if text.lines().count() > 100 {
            // TODO split the content into history and temp_storage
            // for output when the output is very large
            self.content = text_editor::Content::new();
            self.last_line = 0;
        } else {
            self.content
                .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
            if !append {
                self.last_line = self.content.cursor().position.line;
            }
        }
        let text = if prompt {
            let mut new_lines: Vec<_> = if self.residue.is_empty() {
                let mut nl = Vec::with_capacity(text.lines().count());
                let mut lines = text.lines();
                if let Some(l) = lines.next() {
                    nl.push(format!(">>> {l}"))
                }
                nl.extend(lines.map(|l| format!(">.. {l}")));
                nl
            } else {
                text.lines().map(|l| format!(">.. {l}")).collect()
            };
            new_lines.push(String::new()); // for trailing newline
            new_lines.join("\n")
        } else {
            format!("{}\n", text.trim())
        };
        // let lines = text.lines().count();
        self.content
            .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new(text),
            )));
        // workaround to trigger highlight, if using the cargo version instead of patched version
        // for _ in 0..=lines {
        //     self.content
        //         .perform(text_editor::Action::Move(text_editor::Motion::Up));
        // }
        // self.content
        //     .perform(text_editor::Action::Move(text_editor::Motion::Home));
        // self.content
        //     .perform(text_editor::Action::Edit(text_editor::Edit::Enter));
        // self.content
        //     .perform(text_editor::Action::Edit(text_editor::Edit::Backspace));
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
            Message::ClearContent => {
                self.content = text_editor::Content::new();
                self.last_line = 0;
            }
            Message::ClearCommand => {
                self.residue.clear();
                self.command.clear();
            }
            Message::CommandChange(cmd) => {
                self.command = cmd;
            }
            Message::NetworkTyChange(ty) => {
                self.network_ty = ty.clone();
                let _ = self.sender.send(TaskCtxRequest::NetworkTy(ty));
            }
            Message::GetCompletions(compl) => {
                let _ = self.sender.send(TaskCtxRequest::Completions(compl));
            }
            Message::TaskChain(done, mut tasks) => {
                let task = if let Some(t) = tasks.pop() {
                    t
                } else {
                    return Task::none();
                };
                let _ = self.sender.send(TaskCtxRequest::Run(Box::new(task)));
                return Task::perform(async { tasks }, move |t| Message::TaskChain(done + 1, t));
            }
            Message::RunTasks(tasks) => {
                self.append_term(tasks.trim(), true, false);
                let tasks = if self.residue.is_empty() {
                    tasks
                } else {
                    format!("{}\n{}", self.residue, tasks)
                };
                let tokens = nadi_core::parser::tokenizer::get_tokens(&tasks);
                match nadi_core::parser::tokenizer::Token::validate(tokens.clone()) {
                    Ok(tkns) => {
                        use nadi_core::parser::tokenizer::ParenCheck;
                        match ParenCheck::scan(&tkns) {
                            ParenCheck::Unpaired(_) => {
                                // TODO: need to show the prompt that
                                // shows more inputs needed, the
                                // prompting method needs to be
                                // modified for that. instead of
                                // prompt showing with the inputs,
                                // make it show at the end of task
                                // execution.
                                self.residue = tasks;
                                self.status = "Waiting more input...".to_string();
                            }
                            // ParenCheck::Paired
                            // Easier to just get the error from task parsing even if we know the pairs are incorrect (for now)
                            _ => {
                                let tasks_vec = match nadi_core::parser::tasks::parse(tokens) {
                                    Ok(t) => t,
                                    Err(e) => {
                                        self.residue.clear();
                                        self.status = e.to_string();
                                        self.append_term(&e.user_msg(None), false, true);
                                        return Task::none();
                                    }
                                };
                                self.residue.clear();
                                self.append_history(tasks);
                                return Task::perform(
                                    async { tasks_vec.into_iter().rev().collect() },
                                    move |t| Message::TaskChain(0, t),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        self.residue.clear();
                        self.status = e.to_string();
                        self.append_term(&e.user_msg(None), false, true);
                    }
                }
                return Task::none();
            }
            Message::NodeClicked(None) => {
                let _ = self.sender.send(TaskCtxRequest::NetworkAttr);
            }
            Message::NodeClicked(Some(node)) => {
                let _ = self.sender.send(TaskCtxRequest::NodeAttr(node));
            }
            Message::ScaleChanged(scale) => {
                self.network_view.scale = scale / 100.0;
            }
            Message::Tick => {
                let messages: Vec<TaskCtxMessage> = self.receiver.try_iter().collect();
                for m in messages {
                    match m {
                        TaskCtxMessage::Network(nd) => self.network_view.update(nd),
                        TaskCtxMessage::Attribute(name, at) => {
                            return Task::perform(async { (name, at) }, Message::AttrFound);
                        }
                        TaskCtxMessage::Result(out, res) => {
                            self.append_term(out.trim(), false, true);
                            match res {
                                Ok(Some(s)) => self.append_term(s.trim(), false, true),
                                Err(s) => self.append_term(s.trim(), false, true),
                                _ => (),
                            };
                        }
                        TaskCtxMessage::Update(TaskMessage::Progress(label, a, b)) => {
                            self.progress = (label, a as f32 / b as f32 * 100.0);
                            self.running_msg = Some(format!(
                                "Executing Tasks: {:.2}% ({})",
                                self.progress.1, self.progress.0
                            ));
                        }
                        TaskCtxMessage::Update(TaskMessage::Image(img)) => {
                            return Task::perform(async { img }, Message::OpenImage);
                        }
                        // TODO instead of term being an editor, will
                        // make it rich text and manage these better
                        TaskCtxMessage::Update(TaskMessage::Info(s) | TaskMessage::Warning(s)) => {
                            self.append_term(&s, false, true);
                        }
                        // this will trigger when the values are changed in the context
                        TaskCtxMessage::Update(TaskMessage::Changed) => {
                            _ = self.sender.send(TaskCtxRequest::NetworkData);
                        }
                        TaskCtxMessage::Clear => {
                            self.content = text_editor::Content::new();
                            self.residue.clear();
                            self.command.clear();
                            self.network_view.update(NetworkData::default());
                        }
                        TaskCtxMessage::Waiting => {
                            self.progress = (String::new(), 100.0);
                            self.running_msg = None;
                        }
                        TaskCtxMessage::Completions(compls) => {
                            return Task::done(Message::ComplResult(compls));
                        }
                    }
                }
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
            icons::danger_action(icons::trash_icon(), "Clear", Some(Message::ClearContent)),
            icons::action(icons::top_icon(), "Goto Top", Some(Message::GotoTop)),
            icons::action(icons::up_icon(), "Go Up", Some(Message::GoUp)),
            icons::action(icons::down_icon(), "Go Down", Some(Message::GoDown)),
            icons::action(
                icons::bottom_icon(),
                "Goto Bottom",
                Some(Message::GotoBottom)
            ),
            horizontal(),
            combo_box(&self.history, "Search History", None, Message::History)
        ];
        if !self.embedded {
            controls = controls.push(toggler(self.light_theme).on_toggle(Message::ThemeChange));
        }
        let entry = row![
            icons::danger_action(
                icons::trash_icon(),
                "Clear",
                (self.residue.is_empty() && self.command.is_empty())
                    .not()
                    .then(|| Message::ClearCommand)
            ),
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
        ]
        .spacing(10);
        column![
            controls.spacing(10).padding(10),
            text_editor(&self.content)
                .height(Fill)
                .font(Font::MONOSPACE)
                .on_action(Message::EditorAction)
                .highlight_with::<my_hl::NadiHighlighter>(
                    (NadiFileType::Terminal, self.last_line),
                    my_hl::hlto_format
                ),
            text(&self.status).style(text::danger),
            entry,
            progress_bar(0.0..=100.0, self.progress.1)
                .girth(4.0)
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
                    row![
                        text("Network Type:"),
                        pick_list(
                            NetworkViewType::all(),
                            Some(&self.network_ty),
                            Message::NetworkTyChange
                        )
                    ]
                    .spacing(15),
                    row![
                        text("Zoom: "),
                        slider(
                            5.0..=200.0,
                            self.network_view.scale * 100.0,
                            Message::ScaleChanged
                        ),
                        mouse_area(text(format!("{:.1}%", self.network_view.scale * 100.0)))
                            .on_press(Message::ScaleChanged(100.0)),
                    ],
                    scrollable(
                        markdown::view(
                            &self.network_help,
                            markdown::Settings::with_style(md_style(self.light_theme)),
                        )
                        .map(Message::LinkClicked)
                    )
                    .width(Length::FillPortion(1)),
                ]
                .padding(10)
                .spacing(10),
            );
        }
        row![
            scrollable(
                container(NetworkTable::new(&self.network_view).on_press(Message::NodeClicked))
                    .padding(10.0)
                    .width(Length::Fill)
            )
            .width(Length::FillPortion(1))
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

    pub fn subscription(&self) -> Subscription<Message> {
        time::every(Duration::from_millis(100)).map(|_| Message::Tick)
    }
}

fn task_completion(ctx: &TaskContext, start: String) -> Vec<Completion> {
    // TODO, complete more things
    match start.split_once('.') {
        Some((pre, compl)) => match pre {
            "env" => completions(
                ctx.env
                    .attr_map()
                    .keys()
                    .map(|k| (k, &ComplType::Attr))
                    .chain(
                        ctx.functions
                            .env_alias()
                            .keys()
                            .map(|k| (k, &ComplType::Func)),
                    ),
                compl,
            ),
            "network" => completions(
                ctx.network
                    .attr_map()
                    .keys()
                    .map(|k| (k, &ComplType::Attr))
                    .chain(
                        ctx.functions
                            .network_alias()
                            .keys()
                            .map(|k| (k, &ComplType::Func)),
                    ),
                compl,
            ),
            "node" => completions(
                ctx.functions
                    .node_alias()
                    .keys()
                    .map(|k| (k, &ComplType::Func)),
                compl,
            ),
            plugin => {
                if let Some(p) = ctx.functions.plugins().get(plugin) {
                    completions(
                        p.env()
                            .iter()
                            .chain(p.node())
                            .chain(p.network())
                            .map(|k| (k, &ComplType::Func)),
                        compl,
                    )
                } else {
                    vec![]
                }
            }
        },
        None => completions(
            ctx.env
                .attr_map()
                .keys()
                .chain(ctx.network.attr_map().keys())
                .map(|k| (k, &ComplType::Attr)),
            &start,
        ),
    }
}

enum ComplType {
    Func,
    Attr,
}

impl ComplType {
    fn end(&self) -> &str {
        match self {
            Self::Func => "()",
            Self::Attr => " ",
        }
    }

    fn mback(&self) -> usize {
        match self {
            Self::Func => 1,
            Self::Attr => 0,
        }
    }

    fn label(&self, name: &str) -> String {
        match self {
            Self::Func => format!("{name} <f>"),
            Self::Attr => name.to_string(),
        }
    }
}

fn completions<'a, I: Iterator<Item = (&'a RString, &'static ComplType)>>(
    candidates: I,
    start: &str,
) -> Vec<Completion> {
    let compl: Vec<_> = candidates
        .filter(|k| k.0.starts_with(start))
        .map(|k| Completion {
            label: k.1.label(k.0),
            delete: 0,
            content: format!("{}{}", k.0, k.1.end()),
            insert_from: start.len(),
            move_back: k.1.mback(),
        })
        .collect();
    if let [m] = compl.as_slice()
        && m.label == start
    {
        vec![]
    } else {
        compl
    }
}
