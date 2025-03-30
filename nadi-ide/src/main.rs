use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    button, center, column, container, horizontal_space, pick_list, row, text, toggler,
};
use iced::{Element, Fill, Length, Task, Theme};
use nadi::attributes::AttrView;
use nadi::editor::{self, Editor};
use nadi::help::{self, MdHelp};
use nadi::icons;
use nadi::style;
use nadi::svg::SvgView;
use nadi::terminal::{self, Terminal};
use nadi_core::attrs::HasAttributes;

pub fn main() -> iced::Result {
    iced::application("NADI", MainWindow::update, MainWindow::view)
        .font(icons::FONT)
        .theme(MainWindow::theme)
        .run()
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
        Self {
            light_theme: false,
            panes,
            focus: None,
            funchelp: MdHelp::default().embed(),
            editor: Editor::default().embed(),
            svg: SvgView::default().embed(),
            terminal: Terminal::default().embed(),
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
                nadi::terminal::Message::NodeClicked(None) => {
                    self.spawn_pane_maybe(Some(PaneType::AttrView));
                    self.attrs.load_attrs(
                        "Network".to_string(),
                        self.terminal.task_ctx.network.attr_map(),
                    );
                }
                nadi::terminal::Message::NodeClicked(Some(node)) => {
                    self.spawn_pane_maybe(Some(PaneType::AttrView));
                    if let Some(node) = self.terminal.task_ctx.network.node_by_name(&node) {
                        let n = node.lock();
                        self.attrs
                            .load_attrs(format!("Node[{}]: {}", n.index(), n.name()), n.attr_map());
                    }
                }
                _ => return self.terminal.update(m).map(Message::Terminal),
            },
            Message::SvgView(m) => return self.svg.update(m).map(Message::SvgView),
            Message::Attributes => (),
            Message::Editor(m) => {
                return match m {
                    editor::Message::RunAllTask => {
                        let buf = self.editor.content.text();
                        self.spawn_pane_maybe(Some(PaneType::Terminal));
                        Task::perform(async { buf }, terminal::Message::RunTasks)
                            .map(Message::Terminal)
                    }
                    editor::Message::RunTask => {
                        if let Some(sel) = self.editor.content.selection() {
                            self.spawn_pane_maybe(Some(PaneType::Terminal));
                            Task::perform(async { sel }, terminal::Message::RunTasks)
                                .map(Message::Terminal)
                        } else {
                            Task::none()
                        }
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
                        if let Some(func) = self.editor.function.clone() {
                            self.spawn_pane_maybe(Some(PaneType::FunctionHelp));
                            Task::perform(async { func }, |(t, f)| help::Message::Function(t, f))
                                .map(Message::FuncHelp)
                        } else {
                            Task::none()
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

    fn view(&self) -> Element<Message> {
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
        let controls = row![
            horizontal_space(),
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
    Attributes,
    Workspace(pane_grid::Configuration<&'static PaneType>),
    PaneAction(PaneMessage),
    PaneTypeChanged(pane_grid::Pane, PaneType),
    FuncHelp(nadi::help::Message),
    Editor(nadi::editor::Message),
    SvgView(nadi::svg::Message),
    Terminal(nadi::terminal::Message),
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
        Some(PaneType::AttrView) => win.attrs.view().map(|_| Message::Attributes),
    }
}

fn initial_view(win: &MainWindow, id: pane_grid::Pane) -> Element<Message> {
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
        center(
            row![
                col,
                column![
                    center(text("Workspace Layout"))
                        .width(Length::Fill)
                        .height(30.0),
                    button(center("Editor + Terminal"))
                        .on_press_with(|| {
                            Message::Workspace(pane_grid::Configuration::Split {
                                axis: pane_grid::Axis::Vertical,
                                ratio: 0.5,
                                a: Box::new(pane_grid::Configuration::Pane(&PaneType::TextEditor)),
                                b: Box::new(pane_grid::Configuration::Pane(&PaneType::Terminal)),
                            })
                        })
                        .width(Length::Fill)
                        .height(30.0),
                    button(center("Editor + Help / Terminal"))
                        .on_press_with(|| {
                            Message::Workspace(pane_grid::Configuration::Split {
                                axis: pane_grid::Axis::Vertical,
                                ratio: 0.5,
                                a: Box::new(pane_grid::Configuration::Pane(&PaneType::TextEditor)),
                                b: Box::new(pane_grid::Configuration::Split {
                                    axis: pane_grid::Axis::Horizontal,
                                    ratio: 0.5,
                                    a: Box::new(pane_grid::Configuration::Pane(
                                        &PaneType::FunctionHelp,
                                    )),
                                    b: Box::new(pane_grid::Configuration::Pane(
                                        &PaneType::Terminal,
                                    )),
                                }),
                            })
                        })
                        .width(Length::Fill)
                        .height(30.0),
                    button(center("Editor + Svg / Terminal"))
                        .on_press_with(|| {
                            Message::Workspace(pane_grid::Configuration::Split {
                                axis: pane_grid::Axis::Vertical,
                                ratio: 0.5,
                                a: Box::new(pane_grid::Configuration::Pane(&PaneType::TextEditor)),
                                b: Box::new(pane_grid::Configuration::Split {
                                    axis: pane_grid::Axis::Horizontal,
                                    ratio: 0.5,
                                    a: Box::new(pane_grid::Configuration::Pane(&PaneType::SvgView)),
                                    b: Box::new(pane_grid::Configuration::Pane(
                                        &PaneType::Terminal,
                                    )),
                                }),
                            })
                        })
                        .width(Length::Fill)
                        .height(30.0),
                    button(center("Editor + Network / Terminal"))
                        .on_press_with(|| {
                            Message::Workspace(pane_grid::Configuration::Split {
                                axis: pane_grid::Axis::Vertical,
                                ratio: 0.5,
                                a: Box::new(pane_grid::Configuration::Pane(&PaneType::TextEditor)),
                                b: Box::new(pane_grid::Configuration::Split {
                                    axis: pane_grid::Axis::Horizontal,
                                    ratio: 0.5,
                                    a: Box::new(pane_grid::Configuration::Pane(
                                        &PaneType::NetworkView,
                                    )),
                                    b: Box::new(pane_grid::Configuration::Pane(
                                        &PaneType::Terminal,
                                    )),
                                }),
                            })
                        })
                        .width(Length::Fill)
                        .height(30.0),
                    button(center("Editor + Attributes / Terminal"))
                        .on_press_with(|| {
                            Message::Workspace(pane_grid::Configuration::Split {
                                axis: pane_grid::Axis::Vertical,
                                ratio: 0.5,
                                a: Box::new(pane_grid::Configuration::Pane(&PaneType::TextEditor)),
                                b: Box::new(pane_grid::Configuration::Split {
                                    axis: pane_grid::Axis::Horizontal,
                                    ratio: 0.5,
                                    a: Box::new(pane_grid::Configuration::Pane(
                                        &PaneType::AttrView,
                                    )),
                                    b: Box::new(pane_grid::Configuration::Pane(
                                        &PaneType::Terminal,
                                    )),
                                }),
                            })
                        })
                        .width(Length::Fill)
                        .height(30.0)
                ]
                .spacing(10.0)
                .width(300.0)
            ]
            .spacing(30.0),
        )
        .into()
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
