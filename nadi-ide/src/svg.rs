use iced::widget::{
    button, column, horizontal_space, markdown, row, scrollable, svg, text, text_editor,
    text_input, toggler,
};
use iced::{Color, Element, Fill, Font, Length, Task, Theme, widget::Column};
use std::path::PathBuf;

#[derive(Default)]
pub struct SvgView {
    light_theme: bool,
    file: Option<PathBuf>,
    is_loading: bool,
    handle: Option<svg::Handle>,
    embedded: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenFile,
    FileOpened(Option<PathBuf>),
    // Refresh,
    ThemeChange(bool),
}

impl SvgView {
    pub fn embed(mut self) -> Self {
        self.embedded = true;
        self
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChange(theme) => {
                self.light_theme = theme;
                Task::none()
            }
            Message::OpenFile => {
                if self.is_loading {
                    Task::none()
                } else {
                    self.is_loading = true;
                    Task::perform(open_file(), Message::FileOpened)
                }
            }
            Message::FileOpened(path) => {
                self.is_loading = false;
                if let Some(path) = path {
                    self.file = Some(path);
                }
                Task::none()
            } // Message::Refresh => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut controls = row![
            button("Open").on_press(Message::OpenFile),
            // button("Refresh").on_press(Message::Refresh),
            horizontal_space()
        ]
        .spacing(10)
        .padding(10);
        if !self.embedded {
            controls = controls.push(toggler(self.light_theme).on_toggle(Message::ThemeChange));
        }
        let status = row![
            text(
                self.file
                    .as_ref()
                    .map(|p| { p.to_string_lossy().to_string() })
                    .unwrap_or("*No File*".into())
            ),
            horizontal_space()
        ];
        if let Some(h) = &self.file {
            column![controls, svg(h).width(Fill).height(Fill), status]
        } else {
            column![controls, status]
        }
        .padding(10)
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

async fn open_file() -> Option<PathBuf> {
    let path = rfd::AsyncFileDialog::new()
        .set_title("Open a SVG file...")
        .add_filter("SVG", &["svg"])
        .pick_file()
        .await?;
    Some(path.path().into())
}
