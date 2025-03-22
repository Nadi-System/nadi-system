use crate::icons;
use iced::mouse;
use iced::widget::{
    Column, canvas, column, combo_box, container, horizontal_space, hover, row, scrollable, text,
    text_editor, text_input, toggler,
};
use iced::{Color, Element, Fill, Font, Rectangle, Renderer, Task, Theme};
use nadi_core::attrs::{FromAttributeRelaxed, HasAttributes};
use nadi_core::tasks::{Task as NadiTask, TaskContext};
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

pub struct Terminal {
    light_theme: bool,
    is_running: bool,
    history_str: Vec<String>,
    history: combo_box::State<String>,
    command: String,
    status: String,
    content: text_editor::Content,
    pub task_ctx: TaskContext,
    network: Network,
    table_headers: Vec<String>,
    embedded: bool,
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            light_theme: false,
            is_running: false,
            history_str: vec![],
            history: combo_box::State::<String>::default(),
            command: String::new(),
            status: String::new(),
            content: text_editor::Content::default(),
            task_ctx: TaskContext::new(None),
            network: Network::default(),
            table_headers: vec!["NAME".to_string()],
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
    HeaderChange(String),
    HeaderSubmit,
    TasksDone(Result<Option<String>, String>),
    CommandChange(String),
    History(String),
    GotoTop,
    GotoBottom,
    GoUp,
    GoDown,
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
    fn execute_tasks(&mut self, tasks: Vec<NadiTask>) -> (String, Result<Option<String>, String>) {
        // temp solution, make NadiFunctions take a std::io::Write or
        // other trait object that can either print to stdout, or take the
        // result to show somewhere else (like here)
        let mut buf = gag::BufferRedirect::stdout().unwrap();
        let mut output = String::new();
        let mut results = String::new();
        let total = tasks.len();
        // TODO break it into individual tasks and run it with Task::chain
        for (i, fc) in tasks.into_iter().enumerate() {
            let res = self.task_ctx.execute(fc);
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
        }
        (output, Ok(Some(results)))
    }

    fn append_term(&mut self, text: &str) {
        self.content
            .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
        self.content
            .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new(format!("{}\n", text)),
            )));
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        self.status.clear();
        match message {
            Message::ThemeChange(theme) => {
                self.light_theme = theme;
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
            Message::RunTasks(tasks) => {
                self.append_term(&tasks);
                let tasks_vec = match nadi_core::parser::tokenizer::get_tokens(&&tasks) {
                    Ok(tk) => match nadi_core::parser::tasks::parse(tk) {
                        Ok(t) => t,
                        Err(e) => return Task::none(),
                    },
                    Err(e) => return Task::none(),
                };
                let (out, res) = self.execute_tasks(tasks_vec);
                self.append_term(&out);
                match res {
                    Ok(Some(s)) => self.append_term(&s),
                    Err(s) => self.append_term(&s),
                    _ => (),
                };
                self.network
                    .update(&self.task_ctx.network, self.table_headers.clone());
                self.append_history(tasks);
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
                return Task::perform(async { task }, Message::RunTasks);
            }
            Message::HeaderChange(header) => {
                self.table_headers = header.split(';').map(String::from).collect();
            }
            Message::HeaderSubmit => {
                self.network
                    .update(&self.task_ctx.network, self.table_headers.clone());
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
            text_input("Command", &self.command)
                .on_input(Message::CommandChange)
                .on_submit(Message::ExecCommand)
                .font(Font::MONOSPACE),
        ];
        column![
            controls.spacing(10).padding(10),
            text_editor(&self.content)
                .height(Fill)
                .font(Font::MONOSPACE)
                .on_action(Message::EditorAction),
            text(&self.status),
            entry
        ]
        .padding(10)
        .into()
    }

    pub fn view_network(&self) -> Element<'_, Message> {
        column![
            row![
                text_input("Table Headers", &self.table_headers.join(";"))
                    .on_input(Message::HeaderChange)
                    .on_submit(Message::HeaderSubmit),
                text("")
            ]
            .spacing(10.0),
            container(canvas(&self.network).height(Fill).width(Fill))
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

struct Node {
    index: usize,
    name: String,
    size: f32,
    pos: (f32, f32),
    color: Color,
    linecolor: Color,
    data: Vec<String>,
}

impl Node {
    fn new(node: &nadi_core::prelude::NodeInner, data: &[String]) -> Self {
        let size = node
            .attr(nadi_core::graphics::node::NODE_SIZE.0)
            .and_then(f64::from_attr_relaxed)
            .unwrap_or(nadi_core::graphics::node::NODE_SIZE.1) as f32;
        let c = node
            .try_attr::<nadi_core::graphics::color::AttrColor>(
                nadi_core::graphics::node::NODE_COLOR.0,
            )
            .unwrap_or_default()
            .color()
            .unwrap_or(nadi_core::graphics::node::NODE_COLOR.1);
        let color = Color::new(c.r as f32, c.g as f32, c.b as f32, 1.0);
        let c = node
            .try_attr::<nadi_core::graphics::color::AttrColor>(
                nadi_core::graphics::node::LINE_COLOR.0,
            )
            .unwrap_or_default()
            .color()
            .unwrap_or(nadi_core::graphics::node::LINE_COLOR.1);
        let linecolor = Color::new(c.r as f32, c.g as f32, c.b as f32, 1.0);
        let data = data
            .iter()
            .map(|d| node.attr(d).map(|a| a.to_string()).unwrap_or_default())
            .collect();
        Self {
            index: node.index(),
            name: node.name().to_string(),
            size,
            pos: (node.level() as f32, node.index() as f32),
            color,
            linecolor,
            data,
        }
    }
}

struct Network {
    nodes: Vec<Node>,
    edges: Vec<(usize, usize)>,
    data: Vec<String>,
    deltax: f32,
    deltay: f32,
    offsetx: f32,
    offsety: f32,
    deltacol: f32,
    invert: bool,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            nodes: vec![],
            edges: vec![],
            data: vec![],
            deltax: 20.0,
            deltay: 20.0,
            offsetx: 50.0,
            offsety: 50.0,
            deltacol: 20.0,
            invert: true,
        }
    }
}

impl Network {
    fn new(net: &nadi_core::prelude::Network, data: Vec<String>) -> Self {
        let nodes = net.nodes().map(|n| Node::new(&n.lock(), &data)).collect();
        let edges = net
            .nodes()
            .filter_map(|n| {
                let n = n.lock();
                n.output().map(|o| (n.index(), o.lock().index())).into()
            })
            .collect();

        Self {
            nodes,
            edges,
            data,
            ..Default::default()
        }
    }

    fn update(&mut self, net: &nadi_core::prelude::Network, data: Vec<String>) {
        let nodes = net.nodes().map(|n| Node::new(&n.lock(), &data)).collect();
        let edges = net
            .nodes()
            .filter_map(|n| {
                let n = n.lock();
                n.output().map(|o| (n.index(), o.lock().index())).into()
            })
            .collect();
        self.data = data;
        self.nodes = nodes;
        self.edges = edges;
    }
}

impl canvas::Program<Message> for Network {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let curr_node = cursor.position_in(bounds).and_then(|pt| {
            let y = ((pt.y - self.offsety) / self.deltay).round() - 1.0;
            if y < 0.0 {
                None
            } else {
                self.nodes.get(y as usize).map(|n| n.name.as_str())
            }
        });
        let coords: Vec<(f32, f32)> = self
            .nodes
            .iter()
            .map(|n| {
                let (x, y) = n.pos;
                (
                    (x + 1.0) * self.deltax + self.offsetx,
                    (y + 1.0) * self.deltay + self.offsety,
                )
            })
            .collect();

        for (i, data) in self.data.iter().enumerate() {
            let mut txt = canvas::Text::from(data.as_str());
            txt.position = (self.deltax * 5.0 + (i + 1) as f32, self.offsety).into();
            txt.vertical_alignment = iced::alignment::Vertical::Center;
            frame.fill_text(txt);
        }

        for ((from, to), node) in self.edges.iter().zip(&self.nodes) {
            if Some(node.name.as_str()) == curr_node {
                // highlight the row
                frame.fill_rectangle(
                    (0.0, coords[node.index].1 - self.deltay / 2.0).into(),
                    iced::Size::new(bounds.size().width, self.deltay),
                    canvas::Fill {
                        style: canvas::Style::Solid(Color::new(0.8, 0.8, 0.8, 0.4)),
                        ..canvas::Fill::default()
                    },
                );
            }
            let line = canvas::Path::line(coords[*from].into(), coords[*to].into());
            frame.stroke(
                &line,
                canvas::Stroke::default()
                    .with_width(1.5)
                    .with_color(node.linecolor),
            );
        }
        for (node, pos) in self.nodes.iter().zip(coords) {
            let circle = canvas::Path::circle(pos.into(), node.size);
            frame.fill(&circle, node.color);
            for (i, data) in node.data.iter().enumerate() {
                let mut txt = canvas::Text::from(data.as_str());
                txt.position = (self.deltax * 5.0 * (i + 1) as f32, pos.1).into();
                txt.vertical_alignment = iced::alignment::Vertical::Center;
                frame.fill_text(txt);
            }
        }

        // Then, we produce the geometry
        vec![frame.into_geometry()]
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    DialogClosed,
    IoError(std::io::ErrorKind),
}

async fn open_file() -> Result<(PathBuf, Arc<String>), Error> {
    let path = rfd::AsyncFileDialog::new()
        .set_title("Open a SVG file...")
        .add_filter("SVG", &["svg"])
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;
    load_file(path).await
}

async fn load_file(path: impl Into<PathBuf>) -> Result<(PathBuf, Arc<String>), Error> {
    let path = path.into();

    let contents = tokio::fs::read_to_string(&path)
        .await
        .map(Arc::new)
        .map_err(|error| Error::IoError(error.kind()))?;

    Ok((path, contents))
}
