use clap::Parser;
use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    Row, button, center, column, container, pick_list, row, space::horizontal, text, text_editor,
    toggler, tooltip,
};
use iced::{Element, Fill, Length, Subscription, Task, Theme};
use nadi_core::functions::NadiFunctions;
use nadi_ide::attributes::AttrView;
use nadi_ide::editor::{self, Editor};
use nadi_ide::help::{self, MdHelp};
use nadi_ide::icons;
use nadi_ide::style;
use nadi_ide::svg::{Message as SvgMessage, SvgView};
use nadi_ide::terminal::{self, Terminal};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct NadiIdeOptions {
    /// Start NADI IDE in light theme
    #[arg(short, long, action)]
    light_theme: bool,
    /// Tasks file to open during startup
    #[arg(value_name = "TASK_FILE")]
    task_file: Option<PathBuf>,
}

pub fn main() -> iced::Result {
    iced::application(boot, MainWindow::update, MainWindow::view)
        .font(icons::FONT)
        .theme(MainWindow::theme)
        .subscription(MainWindow::subscription)
        .run()
}

fn boot() -> (MainWindow, Task<Message>) {
    let options = NadiIdeOptions::parse();
    let mut ide = MainWindow {
        light_theme: options.light_theme,
        ..Default::default()
    };
    let task = if let Some(t) = options.task_file {
        // if a File is given start with the ENT configuration
        ide.panes = pane_grid::State::<Pane>::with_configuration(panety_2_pane(
            &pane_grid::Configuration::Split {
                axis: pane_grid::Axis::Vertical,
                ratio: 0.5,
                a: Box::new(pane_grid::Configuration::Pane(&PaneType::TextEditor)),
                b: Box::new(pane_grid::Configuration::Split {
                    axis: pane_grid::Axis::Horizontal,
                    ratio: 0.5,
                    a: Box::new(pane_grid::Configuration::Pane(&PaneType::NetworkView)),
                    b: Box::new(pane_grid::Configuration::Pane(&PaneType::Terminal)),
                }),
            },
        ));
        Task::perform(async { editor::Message::OpenFilePath(t) }, Message::Editor)
    } else {
        Task::none()
    };
    (ide, task)
}

struct MainWindow {
    light_theme: bool,
    panes: pane_grid::State<Pane>,
    focus: Option<pane_grid::Pane>,
    funchelp: MdHelp,
    editor: Editor,
    svg: SvgView,
    terminal: Terminal,
    attrs: AttrView,
}

impl Default for MainWindow {
    fn default() -> Self {
        let (panes, _) = pane_grid::State::new(Pane::new());
        let funcs = Some(NadiFunctions::new());
        Self {
            light_theme: false,
            panes,
            focus: None,
            funchelp: MdHelp::new(funcs.clone()).embed(),
            editor: Editor::new(funcs).embed(),
            svg: SvgView::default().embed(),
            terminal: Terminal::new().embed(),
            attrs: AttrView::default(),
        }
    }
}

impl MainWindow {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChange(t) => {
                self.light_theme = t;
                self.funchelp.light_theme = t;
                self.terminal.light_theme = t;
                self.svg.light_theme = t;
            }
            Message::Workspace(conf) => {
                self.panes = pane_grid::State::<Pane>::with_configuration(panety_2_pane(&conf));
            }
            Message::Terminal(m) => match m {
                nadi_ide::terminal::Message::AttrFound((name, am)) => {
                    self.spawn_pane_maybe(Some(PaneType::AttrView));
                    self.attrs.load_attrs(name, &am);
                }
                nadi_ide::terminal::Message::ComplResult(compls) => {
                    return Task::done(editor::Message::ComplResult(compls)).map(Message::Editor);
                }
                nadi_ide::terminal::Message::OpenImage(img) => {
                    return Task::done(SvgMessage::OpenThisFile(img)).map(Message::SvgView);
                }
                _ => return self.terminal.update(m).map(Message::Terminal),
            },
            Message::SvgView(m) => return self.svg.update(m).map(Message::SvgView),
            Message::Attributes(m) => self.attrs.update(m),
            Message::Editor(m) => {
                return match m {
                    editor::Message::RunAllTask => {
                        let buf = self.editor.content.text();
                        self.spawn_pane_maybe(Some(PaneType::Terminal));
                        Task::perform(async { buf }, terminal::Message::RunTasks)
                            .map(Message::Terminal)
                    }
                    editor::Message::RunTask => {
                        let tasks = match self.editor.content.selection() {
                            Some(sel) => sel,
                            None => {
                                let line = self.editor.content.cursor().position.line;
                                self.editor
                                    .content
                                    .perform(text_editor::Action::Move(text_editor::Motion::Down));
                                self.editor
                                    .content
                                    .line(line)
                                    .map(|l| l.text.to_string())
                                    .unwrap_or_default()
                            }
                        };
                        self.spawn_pane_maybe(Some(PaneType::Terminal));
                        Task::perform(async { tasks }, terminal::Message::RunTasks)
                            .map(Message::Terminal)
                    }
                    editor::Message::SearchHelp => {
                        if let Some(sel) = self.editor.content.selection() {
                            self.spawn_pane_maybe(Some(PaneType::FunctionHelp));
                            Task::perform(async { sel }, help::Message::SearchChange)
                                .map(Message::FuncHelp)
                        } else {
                            Task::none()
                        }
                    }
                    editor::Message::HelpTask => {
                        self.spawn_pane_maybe(Some(PaneType::FunctionHelp));
                        if let Some(func) = &self.editor.curr_func {
                            let func = (func.ity.clone(), func.name.clone());
                            Task::perform(async { func }, |(t, f)| help::Message::Function(t, f))
                                .map(Message::FuncHelp)
                        } else {
                            Task::none()
                        }
                    }
                    editor::Message::GetCompletions(compl) => {
                        if let Some(com) = compl
                            && !com.is_empty()
                        {
                            Task::done(terminal::Message::GetCompletions(com))
                                .map(Message::Terminal)
                        } else {
                            Task::done(editor::Message::ComplResult(vec![])).map(Message::Editor)
                        }
                    }
                    _ => self.editor.update(m).map(Message::Editor),
                };
            }
            Message::FuncHelp(m) => self.funchelp.update(m),
            Message::PaneTypeChanged(p, typ) => {
                if let Some(Pane { ty, .. }) = self.panes.get_mut(p) {
                    *ty = Some(typ);
                }
            }
            Message::PaneAction(m) => match m {
                PaneMessage::Split(axis, pane) => {
                    let result = self.panes.split(axis, pane, Pane::new());

                    if let Some((pane, _)) = result {
                        self.focus = Some(pane);
                    }
                }
                PaneMessage::Clicked(pane) => {
                    self.focus = Some(pane);
                }
                PaneMessage::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                    self.panes.resize(split, ratio);
                }
                PaneMessage::Dragged(pane_grid::DragEvent::Dropped { pane, target }) => {
                    self.panes.drop(pane, target);
                }
                PaneMessage::Dragged(_) => {}
                PaneMessage::TogglePin(pane) => {
                    if let Some(Pane { is_pinned, .. }) = self.panes.get_mut(pane) {
                        *is_pinned = !*is_pinned;
                    }
                }
                PaneMessage::Maximize(pane) => self.panes.maximize(pane),
                PaneMessage::Restore => {
                    self.panes.restore();
                }
                PaneMessage::Close(pane) => {
                    if let Some((_, sibling)) = self.panes.close(pane) {
                        self.focus = Some(sibling);
                    }
                }
            },
        }
        Task::none()
    }

    fn view<'a>(&'a self) -> Element<'a, Message> {
        let focus = self.focus;
        let pane_grid = PaneGrid::new(&self.panes, |id, pane, is_maximized| {
            let is_focused = focus == Some(id);
            let pin_button = icons::action(
                if pane.is_pinned {
                    icons::unpin_icon()
                } else {
                    icons::pin_icon()
                },
                if pane.is_pinned { "Unpin" } else { "Pin" },
                Some(Message::PaneAction(PaneMessage::TogglePin(id))),
            );
            let title = row![
                pin_button,
                text(
                    pane.ty
                        .map(|t| t.to_string())
                        .unwrap_or("Choose Pane Type".into())
                ),
            ]
            .spacing(5);
            let title_bar = pane_grid::TitleBar::new(title)
                .controls(pane_controls(
                    id,
                    pane,
                    self.panes.panes.len(),
                    is_maximized,
                ))
                .padding(1)
                .style(if is_focused {
                    style::title_bar_focused
                } else {
                    style::title_bar_active
                });
            pane_grid::Content::new(pane_content(self, id, &pane.ty))
                .title_bar(title_bar)
                .style(if is_focused {
                    style::pane_focused
                } else {
                    style::pane_active
                })
        })
        .width(Fill)
        .height(Fill)
        .spacing(10)
        .on_click(|p| Message::PaneAction(PaneMessage::Clicked(p)))
        .on_drag(|p| Message::PaneAction(PaneMessage::Dragged(p)))
        .on_resize(10, |p| Message::PaneAction(PaneMessage::Resized(p)));
        let panes = Row::from_iter(workspace_options().into_iter().map(|(name, tip, conf)| {
            tooltip(
                button(center(text(name)))
                    .on_press(Message::Workspace(conf))
                    .height(30.0)
                    .width(60.0),
                tip,
                tooltip::Position::Top,
            )
            .style(container::rounded_box)
            .into()
        }))
        .spacing(10.0);
        let controls = row![
            panes,
            horizontal(),
            icons::action(
                icons::help_icon(),
                "Browse Nadi Book",
                Some(Message::FuncHelp(help::Message::Book))
            ),
            icons::action(
                icons::github_icon(),
                "Visit Github Repository",
                Some(Message::FuncHelp(help::Message::Github))
            ),
            toggler(self.light_theme).on_toggle(Message::ThemeChange),
        ]
        .spacing(20)
        .padding(10);
        column![
            controls,
            container(pane_grid).width(Fill).height(Fill).padding(10),
        ]
        .into()
    }

    fn theme(&self) -> Theme {
        if self.light_theme {
            Theme::Light
        } else {
            Theme::Dark
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.editor.subscription().map(Message::Editor),
            self.terminal.subscription().map(Message::Terminal),
        ])
    }

    fn spawn_pane_maybe(&mut self, ty: Option<PaneType>) {
        if self.panes.iter().any(|(_, p)| p.ty == ty) {
            return;
        }
        if let Some(pane) = self.focus {
            let mut p = Pane::new();
            p.ty = ty;
            let result = self.panes.split(pane_grid::Axis::Vertical, pane, p);

            if let Some((pane, _)) = result {
                self.focus = Some(pane);
            }
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Attributes(nadi_ide::attributes::Message),
    Workspace(pane_grid::Configuration<&'static PaneType>),
    PaneAction(PaneMessage),
    PaneTypeChanged(pane_grid::Pane, PaneType),
    FuncHelp(nadi_ide::help::Message),
    Editor(nadi_ide::editor::Message),
    SvgView(nadi_ide::svg::Message),
    Terminal(nadi_ide::terminal::Message),
    ThemeChange(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneType {
    FunctionHelp,
    TextEditor,
    SvgView,
    NetworkView,
    Terminal,
    AttrView,
}

impl PaneType {
    pub const ALL: &'static [PaneType] = &[
        PaneType::FunctionHelp,
        PaneType::TextEditor,
        PaneType::SvgView,
        PaneType::NetworkView,
        PaneType::Terminal,
        PaneType::AttrView,
    ];
}

impl std::fmt::Display for PaneType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::FunctionHelp => "Function Help",
                Self::TextEditor => "Text Editor",
                Self::SvgView => "Svg Viewer",
                Self::NetworkView => "Network Viewer",
                Self::Terminal => "Terminal",
                Self::AttrView => "Attributes",
            }
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum PaneMessage {
    Split(pane_grid::Axis, pane_grid::Pane),
    Clicked(pane_grid::Pane),
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    TogglePin(pane_grid::Pane),
    Maximize(pane_grid::Pane),
    Restore,
    Close(pane_grid::Pane),
}

struct Pane {
    pub is_pinned: bool,
    pub ty: Option<PaneType>,
}

impl Pane {
    fn new() -> Self {
        Self {
            is_pinned: false,
            ty: None,
        }
    }
}
fn pane_controls<'a>(
    id: pane_grid::Pane,
    pane: &Pane,
    panes_count: usize,
    is_maximized: bool,
) -> Element<'a, Message> {
    row![
        pick_list(PaneType::ALL, pane.ty, move |t| Message::PaneTypeChanged(
            id, t
        ),),
        icons::action(
            icons::hsplit_icon(),
            "Horizontal Split",
            Some(Message::PaneAction(PaneMessage::Split(
                pane_grid::Axis::Horizontal,
                id
            ))),
        ),
        icons::action(
            icons::vsplit_icon(),
            "Vertical Split",
            Some(Message::PaneAction(PaneMessage::Split(
                pane_grid::Axis::Vertical,
                id
            ))),
        ),
        if is_maximized {
            icons::action(
                icons::resize_small_icon(),
                "Restore",
                Some(Message::PaneAction(PaneMessage::Restore)),
            )
        } else {
            icons::action(
                icons::resize_full_icon(),
                "Maximize",
                (panes_count > 1).then_some(Message::PaneAction(PaneMessage::Maximize(id))),
            )
        },
        icons::danger_action(
            icons::cancel_icon(),
            "Close",
            (panes_count > 1).then_some(Message::PaneAction(PaneMessage::Close(id))),
        ),
    ]
    .spacing(5)
    .into()
}
fn pane_content<'a>(
    win: &'a MainWindow,
    id: pane_grid::Pane,
    ty: &'a Option<PaneType>,
) -> Element<'a, Message> {
    match ty {
        None => initial_view(win, id),
        Some(PaneType::FunctionHelp) => win.funchelp.view().map(Message::FuncHelp),
        Some(PaneType::TextEditor) => win.editor.view().map(Message::Editor),
        Some(PaneType::SvgView) => win.svg.view().map(Message::SvgView),
        Some(PaneType::NetworkView) => win.terminal.view_network().map(Message::Terminal),
        Some(PaneType::Terminal) => win.terminal.view().map(Message::Terminal),
        Some(PaneType::AttrView) => win.attrs.view().map(Message::Attributes),
    }
}

fn initial_view<'a>(win: &'a MainWindow, id: pane_grid::Pane) -> Element<'a, Message> {
    let mut col = column![center(text("Pane Type")).width(Length::Fill).height(30.0),]
        .spacing(10.0)
        .width(300.0);
    for pt in PaneType::ALL {
        col = col.push(
            button(center(text(pt.to_string())))
                .width(Length::Fill)
                .height(30.0)
                .on_press(Message::PaneTypeChanged(id, *pt)),
        );
    }
    if win.panes.panes.len() == 1 {
        let mut col2 = column![
            center(text("Workspace Layout"))
                .width(Length::Fill)
                .height(30.0),
        ]
        .spacing(10.0)
        .width(300.0);
        for (_, name, conf) in workspace_options() {
            col2 = col2.push(
                button(center(text(name)))
                    .width(Length::Fill)
                    .height(30.0)
                    .on_press(Message::Workspace(conf)),
            );
        }
        center(row![col, col2].spacing(30.0)).into()
    } else {
        center(col).into()
    }
}

fn panety_2_pane(conf: &pane_grid::Configuration<&PaneType>) -> pane_grid::Configuration<Pane> {
    match conf {
        pane_grid::Configuration::Pane(ty) => {
            let mut pane = Pane::new();
            pane.ty = Some(**ty);
            pane_grid::Configuration::Pane(pane)
        }
        pane_grid::Configuration::Split { axis, ratio, a, b } => pane_grid::Configuration::Split {
            axis: *axis,
            ratio: *ratio,
            a: Box::new(panety_2_pane(a)),
            b: Box::new(panety_2_pane(b)),
        },
    }
}

fn workspace_options() -> Vec<(
    &'static str,
    &'static str,
    pane_grid::Configuration<&'static PaneType>,
)> {
    vec![
        (
            "ET",
            "Editor + Terminal",
            pane_grid::Configuration::Split {
                axis: pane_grid::Axis::Vertical,
                ratio: 0.5,
                a: Box::new(pane_grid::Configuration::Pane(&PaneType::TextEditor)),
                b: Box::new(pane_grid::Configuration::Pane(&PaneType::Terminal)),
            },
        ),
        (
            "ENT",
            "Editor + Network / Terminal",
            pane_grid::Configuration::Split {
                axis: pane_grid::Axis::Vertical,
                ratio: 0.5,
                a: Box::new(pane_grid::Configuration::Pane(&PaneType::TextEditor)),
                b: Box::new(pane_grid::Configuration::Split {
                    axis: pane_grid::Axis::Horizontal,
                    ratio: 0.5,
                    a: Box::new(pane_grid::Configuration::Pane(&PaneType::NetworkView)),
                    b: Box::new(pane_grid::Configuration::Pane(&PaneType::Terminal)),
                }),
            },
        ),
        (
            "EHT",
            "Editor + Help / Terminal",
            pane_grid::Configuration::Split {
                axis: pane_grid::Axis::Vertical,
                ratio: 0.5,
                a: Box::new(pane_grid::Configuration::Pane(&PaneType::TextEditor)),
                b: Box::new(pane_grid::Configuration::Split {
                    axis: pane_grid::Axis::Horizontal,
                    ratio: 0.5,
                    a: Box::new(pane_grid::Configuration::Pane(&PaneType::FunctionHelp)),
                    b: Box::new(pane_grid::Configuration::Pane(&PaneType::Terminal)),
                }),
            },
        ),
        (
            "EST",
            "Editor + Svg / Terminal",
            pane_grid::Configuration::Split {
                axis: pane_grid::Axis::Vertical,
                ratio: 0.5,
                a: Box::new(pane_grid::Configuration::Pane(&PaneType::TextEditor)),
                b: Box::new(pane_grid::Configuration::Split {
                    axis: pane_grid::Axis::Horizontal,
                    ratio: 0.5,
                    a: Box::new(pane_grid::Configuration::Pane(&PaneType::SvgView)),
                    b: Box::new(pane_grid::Configuration::Pane(&PaneType::Terminal)),
                }),
            },
        ),
        (
            "EAT",
            "Editor + Attributes / Terminal",
            pane_grid::Configuration::Split {
                axis: pane_grid::Axis::Vertical,
                ratio: 0.5,
                a: Box::new(pane_grid::Configuration::Pane(&PaneType::TextEditor)),
                b: Box::new(pane_grid::Configuration::Split {
                    axis: pane_grid::Axis::Horizontal,
                    ratio: 0.5,
                    a: Box::new(pane_grid::Configuration::Pane(&PaneType::AttrView)),
                    b: Box::new(pane_grid::Configuration::Pane(&PaneType::Terminal)),
                }),
            },
        ),
    ]
}
