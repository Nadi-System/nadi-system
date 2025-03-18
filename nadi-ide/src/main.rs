use iced::keyboard;
use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    button, column, container, horizontal_space, pick_list, responsive, row, scrollable, text,
    toggler,
};
use iced::{Center, Color, Element, Fill, Size, Subscription, Task, Theme};

use nadi::editor::Editor;
use nadi::help::MdHelp;
use nadi::icons;
use nadi::style;
use nadi::svg::SvgView;

pub fn main() -> iced::Result {
    iced::application("NADI", MainWindow::update, MainWindow::view)
        .font(include_bytes!("../fonts/icons.ttf").as_slice())
        .theme(MainWindow::theme)
        .run()
}

struct MainWindow {
    light_theme: bool,
    panes: pane_grid::State<Pane>,
    panes_count: usize,
    focus: Option<pane_grid::Pane>,
    funchelp: MdHelp,
    editor: Editor,
    svg: SvgView,
}

impl Default for MainWindow {
    fn default() -> Self {
        let (panes, _) = pane_grid::State::new(Pane::new(0));
        Self {
            light_theme: false,
            panes,
            panes_count: 1,
            focus: None,
            funchelp: MdHelp::default().embed(),
            editor: Editor::default().embed(),
            svg: SvgView::default().embed(),
        }
    }
}

impl MainWindow {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChange(t) => {
                self.light_theme = t;
            }
            Message::SvgView(m) => return self.svg.update(m).map(Message::SvgView),
            Message::Editor(m) => return self.editor.update(m).map(Message::Editor),
            Message::FuncHelp(m) => self.funchelp.update(m),
            Message::PaneTypeChanged(p, typ) => {
                if let Some(Pane { ty, .. }) = self.panes.get_mut(p) {
                    *ty = Some(typ);
                }
            }
            Message::PaneAction(m) => match m {
                PaneMessage::Split(axis, pane) => {
                    let result = self.panes.split(axis, pane, Pane::new(self.panes_count));

                    if let Some((pane, _)) = result {
                        self.focus = Some(pane);
                    }

                    self.panes_count += 1;
                }
                PaneMessage::SplitFocused(axis) => {
                    if let Some(pane) = self.focus {
                        let result = self.panes.split(axis, pane, Pane::new(self.panes_count));

                        if let Some((pane, _)) = result {
                            self.focus = Some(pane);
                        }

                        self.panes_count += 1;
                    }
                }
                PaneMessage::FocusAdjacent(direction) => {
                    if let Some(pane) = self.focus {
                        if let Some(adjacent) = self.panes.adjacent(pane, direction) {
                            self.focus = Some(adjacent);
                        }
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
                PaneMessage::CloseFocused => {
                    if let Some(pane) = self.focus {
                        if let Some(Pane { is_pinned, .. }) = self.panes.get(pane) {
                            if !is_pinned {
                                if let Some((_, sibling)) = self.panes.close(pane) {
                                    self.focus = Some(sibling);
                                }
                            }
                        }
                    }
                }
            },
        }
        Task::none()
    }

    fn view(&self) -> Element<Message> {
        let focus = self.focus;
        let total_panes = self.panes.len();

        let pane_grid = PaneGrid::new(&self.panes, |id, pane, is_maximized| {
            let is_focused = focus == Some(id);
            let pin_button = icons::action(
                icons::pin_icon(),
                "Pin/Unpin",
                Some(Message::PaneAction(PaneMessage::TogglePin(id))),
            );
            let title = row![pin_button, "Pane", text(pane.id.to_string()),].spacing(5);
            let title_bar = pane_grid::TitleBar::new(title)
                .controls(pane_controls(id, pane, self.panes_count, is_maximized))
                .padding(1)
                .style(if is_focused {
                    style::title_bar_focused
                } else {
                    style::title_bar_active
                });
            pane_grid::Content::new(responsive(move |size| {
                pane_content(&self, id, &pane.ty, total_panes, pane.is_pinned, size)
            }))
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
}

#[derive(Debug, Clone)]
enum Message {
    PaneAction(PaneMessage),
    PaneTypeChanged(pane_grid::Pane, PaneType),
    FuncHelp(nadi::help::Message),
    Editor(nadi::editor::Message),
    SvgView(nadi::svg::Message),
    ThemeChange(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneType {
    FunctionHelp,
    TextEditor,
    SvgView,
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
            }
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum PaneMessage {
    Split(pane_grid::Axis, pane_grid::Pane),
    SplitFocused(pane_grid::Axis),
    FocusAdjacent(pane_grid::Direction),
    Clicked(pane_grid::Pane),
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    TogglePin(pane_grid::Pane),
    Maximize(pane_grid::Pane),
    Restore,
    Close(pane_grid::Pane),
    CloseFocused,
}

struct Pane {
    id: usize,
    pub is_pinned: bool,
    pub ty: Option<PaneType>,
}

impl Pane {
    fn new(id: usize) -> Self {
        Self {
            id,
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
        pick_list(
            [
                PaneType::FunctionHelp,
                PaneType::TextEditor,
                PaneType::SvgView
            ],
            pane.ty,
            move |t| Message::PaneTypeChanged(id, t),
        ),
        icons::action(
            "H",
            "Horizontal Split",
            Some(Message::PaneAction(PaneMessage::Split(
                pane_grid::Axis::Horizontal,
                id
            ))),
        ),
        icons::action(
            "V",
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
                (panes_count > 1).then(|| Message::PaneAction(PaneMessage::Maximize(id))),
            )
        },
        icons::danger_action(
            icons::cancel_icon(),
            "Close",
            (panes_count > 1).then(|| Message::PaneAction(PaneMessage::Close(id))),
        ),
    ]
    .spacing(5)
    .into()
}
fn pane_content<'a>(
    win: &'a MainWindow,
    pane: pane_grid::Pane,
    ty: &'a Option<PaneType>,
    total_panes: usize,
    is_pinned: bool,
    size: Size,
) -> Element<'a, Message> {
    match ty {
        None => text("Select a Pane Type").into(),
        Some(PaneType::FunctionHelp) => win.funchelp.view().map(Message::FuncHelp).into(),
        Some(PaneType::TextEditor) => win.editor.view().map(Message::Editor).into(),
        Some(PaneType::SvgView) => win.svg.view().map(Message::SvgView).into(),
    }
}
