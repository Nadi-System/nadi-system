use crate::icons;
use iced::widget::{column, horizontal_space, row, text, text_editor, text_input, toggler};
use iced::{Element, Fill, Font, Task, Theme};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub struct Terminal {
    light_theme: bool,
    is_running: bool,
    history: Vec<String>,
    command: String,
    content: text_editor::Content,
    embedded: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    ThemeChange(bool),
    SaveHistory,
    Run(String),
    CommandChange(String),
    GotoTop,
    GotoBottom,
    GoUp,
    GoDown,
}

impl Terminal {
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
            Message::SaveHistory => Task::none(),
            _ => Task::none(),
        }
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
            horizontal_space()
        ]
        .spacing(10)
        .padding(10);
        if !self.embedded {
            controls = controls.push(toggler(self.light_theme).on_toggle(Message::ThemeChange));
        }
        let entry = row![
            text_input("Command", &self.command)
                .on_input(Message::CommandChange)
                .font(Font::MONOSPACE) // some way to get history here
        ];
        column![
            controls,
            text_editor(&self.content)
                .height(Fill)
                .font(Font::MONOSPACE),
            entry
        ]
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
