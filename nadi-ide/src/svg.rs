use crate::icons;
use iced::widget::{center, column, container, row, space::horizontal, svg, text, toggler};
use iced::{Element, Fill, Task, Theme};
use std::path::PathBuf;
use std::sync::Arc;

pub struct SvgView {
    pub light_theme: bool,
    file: Option<PathBuf>,
    files: Vec<String>,
    curr_ind: usize,
    is_loading: bool,
    handle: svg::Handle,
    embedded: bool,
    err: Option<String>,
}

const DEFAULT_SVG: &[u8; 1791] = include_bytes!("../images/placeholder.svg");

impl Default for SvgView {
    fn default() -> Self {
        Self {
            light_theme: false,
            file: None,
            files: vec![],
            curr_ind: 0,
            is_loading: false,
            handle: svg::Handle::from_memory(DEFAULT_SVG),
            embedded: false,
            err: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenFile,
    OpenThisFile(String),
    FileOpened(Result<(PathBuf, Arc<String>), Error>),
    Refresh,
    ThemeChange(bool),
    NextImage,
    PrevImage,
    ClearImages,
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
            Message::ClearImages => {
                self.curr_ind = 0;
                self.files = vec![];
                self.handle = svg::Handle::from_memory(DEFAULT_SVG);
                Task::none()
            }
            Message::OpenThisFile(img) => {
                if self.is_loading {
                    Task::none()
                } else {
                    self.is_loading = true;
                    self.curr_ind = self.files.len();
                    self.files.push(img.clone());
                    self.file = Some(img.clone().into());
                    Task::perform(load_file(img), Message::FileOpened)
                }
            }
            Message::NextImage => {
                if (self.curr_ind + 1) < self.files.len() {
                    self.curr_ind += 1;
                    if let Some(v) = self.files.get(self.curr_ind) {
                        self.is_loading = true;
                        let v = v.to_string();
                        return Task::perform(load_file(v), Message::FileOpened);
                    }
                }
                Task::none()
            }
            Message::PrevImage => {
                if self.curr_ind > 0 {
                    self.curr_ind -= 1;
                    if let Some(v) = self.files.get(self.curr_ind) {
                        self.is_loading = true;
                        let v = v.to_string();
                        return Task::perform(load_file(v), Message::FileOpened);
                    }
                }
                Task::none()
            }
            Message::FileOpened(result) => {
                self.is_loading = false;
                match result {
                    Ok((path, contents)) => {
                        self.file = Some(path);
                        self.handle =
                            svg::Handle::from_memory(String::clone(&contents).into_bytes());
                        self.err = None;
                    }
                    Err(e) => {
                        self.err = Some(format!("{e:?}"));
                        self.handle = svg::Handle::from_memory(DEFAULT_SVG);
                    }
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
            icons::danger_action(
                icons::trash_icon(),
                "Clear Images",
                Some(Message::ClearImages)
            ),
            horizontal(),
            icons::action(
                icons::left_icon(),
                "Previous Image",
                (self.curr_ind > 0).then(|| Message::PrevImage)
            ),
            icons::action(
                icons::right_icon(),
                "Next Image",
                ((self.curr_ind + 1) < self.files.len()).then(|| Message::NextImage)
            ),
        ]
        .spacing(10)
        .padding(10);
        if !self.embedded {
            controls = controls.push(toggler(self.light_theme).on_toggle(Message::ThemeChange));
        }
        let mut status = row![
            text(
                self.file
                    .as_ref()
                    .map(|p| { p.to_string_lossy().to_string() })
                    .unwrap_or("*No File*".into())
            ),
            horizontal()
        ];
        if let Some(e) = &self.err {
            status = status.push(text(e).color(iced::Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }));
        }
        // if let Some(h) = &self.handle {
        column![
            controls,
            center(
                container(
                    svg(self.handle.clone())
                        .width(iced::Length::Shrink)
                        .height(iced::Length::Shrink)
                )
                .style(|_| {
                    container::Style::default().shadow(iced::Shadow {
                        color: if self.err.is_none() {
                            iced::Color::BLACK
                        } else {
                            iced::Color {
                                r: 1.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }
                        },
                        offset: iced::Vector::new(3.0, 3.0),
                        blur_radius: 10.0,
                    })
                })
                .width(iced::Length::Shrink)
                .height(iced::Length::Shrink)
            )
            .width(Fill)
            .height(Fill),
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
