use crate::icons;
use iced::widget::{column, horizontal_space, row, svg, text, toggler};
use iced::{Element, Fill, Task, Theme};
use std::path::PathBuf;
use std::sync::Arc;

pub struct SvgView {
    light_theme: bool,
    file: Option<PathBuf>,
    is_loading: bool,
    handle: svg::Handle,
    embedded: bool,
}

impl Default for SvgView {
    fn default() -> Self {
        Self {
            light_theme: false,
            file: None,
            is_loading: false,
            handle: svg::Handle::from_memory(include_bytes!("../images/placeholder.svg")),
            embedded: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenFile,
    FileOpened(Result<(PathBuf, Arc<String>), Error>),
    Refresh,
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
            Message::FileOpened(result) => {
                self.is_loading = false;
                match result {
                    Ok((path, contents)) => {
                        self.file = Some(path);
                        self.handle =
                            svg::Handle::from_memory(String::clone(&contents).into_bytes());
                    }
                    Err(e) => println!("{e:?}"),
                }
                Task::none()
            }
            Message::Refresh => {
                if self.is_loading {
                    Task::none()
                } else if let Some(f) = &self.file {
                    self.is_loading = true;
                    Task::perform(load_file(f.clone()), Message::FileOpened)
                } else {
                    Task::none()
                }
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut controls = row![
            icons::action(icons::open_icon(), "Open SVG", Some(Message::OpenFile)),
            icons::action(icons::refresh_icon(), "Refresh", Some(Message::Refresh)),
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
        // if let Some(h) = &self.handle {
        column![
            controls,
            svg(self.handle.clone()).width(Fill).height(Fill),
            status
        ]
        // } else {
        //     column![controls, status]
        // }
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
