use iced::keyboard;
use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    button, column, container, horizontal_space, pick_list, responsive, row, scrollable, text,
};
use iced::{Center, Color, Element, Fill, Size, Subscription, Theme};

use nadi::editor::Editor;
use nadi::help::MdHelp;
use nadi::style;

pub fn main() -> iced::Result {
    iced::application("NADI", MainWindow::update, MainWindow::view)
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
}

impl Default for MainWindow {
    fn default() -> Self {
        let (panes, _) = pane_grid::State::new(Pane::new(0));
        Self {
            light_theme: false,
            panes,
            panes_count: 1,
            focus: None,
            funchelp: MdHelp::default(),
            editor: Editor::default(),
        }
    }
}

impl MainWindow {
    fn update(&mut self, message: Message) {
        match message {
            Message::Editor(m) => self.editor.update(m),
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
    }

    fn view(&self) -> Element<Message> {
        let focus = self.focus;
        let total_panes = self.panes.len();

        let pane_grid = PaneGrid::new(&self.panes, |id, pane, is_maximized| {
            let is_focused = focus == Some(id);
            let pin_button = button(text(if pane.is_pinned { "U" } else { "P" }).size(14))
                .on_press(Message::PaneAction(PaneMessage::TogglePin(id)))
                .padding(3);
            let title = row![
                pin_button,
                "Pane",
                text(pane.id.to_string()).color(if is_focused {
                    PANE_ID_COLOR_FOCUSED
                } else {
                    PANE_ID_COLOR_UNFOCUSED
                }),
            ]
            .spacing(5);
            let title_bar = pane_grid::TitleBar::new(title)
                .controls(pane_controls(id, pane, self.panes_count))
                .padding(10)
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
        container(pane_grid)
            .width(Fill)
            .height(Fill)
            .padding(10)
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Light
    }
}

#[derive(Debug, Clone)]
enum Message {
    PaneAction(PaneMessage),
    PaneTypeChanged(pane_grid::Pane, PaneType),
    FuncHelp(nadi::help::Message),
    Editor(nadi::editor::Message),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneType {
    FunctionHelp,
    TextEditor,
}

impl std::fmt::Display for PaneType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::FunctionHelp => "Function Help",
                Self::TextEditor => "Text Editor",
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
fn pane_controls<'a>(id: pane_grid::Pane, pane: &Pane, panes_count: usize) -> Element<'a, Message> {
    row![
        pick_list(
            [PaneType::FunctionHelp, PaneType::TextEditor],
            pane.ty,
            move |t| Message::PaneTypeChanged(id, t),
        ),
        button("H")
            .padding(3)
            .on_press(Message::PaneAction(PaneMessage::Split(
                pane_grid::Axis::Horizontal,
                id
            ))),
        button("V")
            .padding(3)
            .on_press(Message::PaneAction(PaneMessage::Split(
                pane_grid::Axis::Vertical,
                id
            ))),
        button("X")
            .padding(3)
            .style(button::danger)
            .on_press_maybe(if panes_count > 1 {
                Some(Message::PaneAction(PaneMessage::Close(id)))
            } else {
                None
            })
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
    }
}

const PANE_ID_COLOR_UNFOCUSED: Color = Color::from_rgb(
    0xFF as f32 / 255.0,
    0xC7 as f32 / 255.0,
    0xC7 as f32 / 255.0,
);
const PANE_ID_COLOR_FOCUSED: Color = Color::from_rgb(
    0xFF as f32 / 255.0,
    0x47 as f32 / 255.0,
    0x47 as f32 / 255.0,
);
