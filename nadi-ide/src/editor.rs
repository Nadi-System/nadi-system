use crate::icons;
use iced::widget::{
    button, column, horizontal_space, markdown, row, scrollable, text, text_editor, text_input,
    toggler,
};
use iced::{Color, Element, Fill, Font, Length, Task, Theme, widget::Column};
use nadi_core::functions::{FuncArg, NadiFunctions};
use std::path::{Path, PathBuf};
use std::sync::Arc;

static FUNC_WIDTH: f32 = 300.0;

#[derive(Default)]
pub struct Editor {
    light_theme: bool,
    signature: String,
    file: Option<PathBuf>,
    is_dirty: bool,
    is_loading: bool,
    content: text_editor::Content,
    embedded: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    EditorAction(text_editor::Action),
    ThemeChange(bool),
    NewFile,
    OpenFile,
    FileOpened(Result<(PathBuf, Arc<String>), Error>),
    SaveFile,
    FileSaved(Result<PathBuf, Error>),
}

impl Editor {
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
            Message::EditorAction(action) => {
                self.is_dirty = self.is_dirty || action.is_edit();
                self.content.perform(action);
                Task::none()
            }
            Message::NewFile => {
                if !self.is_loading {
                    self.file = None;
                    self.content = text_editor::Content::new();
                }
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
                self.is_dirty = false;
                match result {
                    Ok((path, contents)) => {
                        self.file = Some(path);
                        self.content = text_editor::Content::with_text(&contents);
                    }
                    Err(e) => {
                        println!("{e:?}")
                    }
                };
                Task::none()
            }
            Message::SaveFile => {
                if self.is_loading {
                    Task::none()
                } else {
                    self.is_loading = true;

                    let mut text = self.content.text();

                    // // only on 0.14
                    // if let Some(ending) = self.content.line_ending() {
                    //     if !text.ends_with(ending.as_str()) {
                    //         text.push_str(ending.as_str());
                    //     }
                    // }
                    if !text.ends_with('\n') {
                        text.push('\n');
                    }

                    Task::perform(save_file(self.file.clone(), text), Message::FileSaved)
                }
            }
            Message::FileSaved(result) => {
                self.is_loading = false;

                match result {
                    Ok(path) => {
                        self.file = Some(path);
                        self.is_dirty = false;
                    }
                    Err(e) => {
                        println!("{e:?}")
                    }
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut controls = row![
            icons::action(icons::trash_icon(), "New", Some(Message::NewFile)),
            icons::action(
                icons::folder_open_empty_icon(),
                "Open",
                Some(Message::OpenFile)
            ),
            icons::action(
                icons::download_icon(),
                "Save",
                self.is_dirty.then(|| Message::SaveFile)
            ),
            horizontal_space()
        ]
        .spacing(10);
        if !self.embedded {
            controls = controls.push(toggler(self.light_theme).on_toggle(Message::ThemeChange));
        }
        let signature = row![text(self.signature.clone())];
        let status = row![
            text(
                self.file
                    .as_ref()
                    .map(|p| { p.to_string_lossy().to_string() })
                    .unwrap_or("*New File*".into())
            ),
            horizontal_space(),
            text({
                let (line, column) = self.content.cursor_position();
                format!("{}:{}", line + 1, column + 1)
            })
        ];
        column![
            controls,
            signature,
            text_editor(&self.content)
                .height(Fill)
                .on_action(Message::EditorAction)
                .font(Font::MONOSPACE)
                .highlight(
                    self.file
                        .as_deref()
                        .and_then(Path::extension)
                        .and_then(std::ffi::OsStr::to_str)
                        .unwrap_or("txt"),
                    iced::highlighter::Theme::SolarizedDark,
                ),
            status
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
    let picked_file = rfd::AsyncFileDialog::new()
        .set_title("Open a text file...")
        .add_filter("Nadi Files", &["net", "network", "tasks", "toml"])
        .add_filter("Text", &["txt", "md", "org", "tex", "html"])
        .add_filter("Code", &["rs", "py"])
        .add_filter("All Files", &["*"])
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;

    load_file(picked_file).await
}

async fn load_file(path: impl Into<PathBuf>) -> Result<(PathBuf, Arc<String>), Error> {
    let path = path.into();

    let contents = tokio::fs::read_to_string(&path)
        .await
        .map(Arc::new)
        .map_err(|error| Error::IoError(error.kind()))?;

    Ok((path, contents))
}

async fn save_file(path: Option<PathBuf>, contents: String) -> Result<PathBuf, Error> {
    let path = if let Some(path) = path {
        path
    } else {
        rfd::AsyncFileDialog::new()
            .save_file()
            .await
            .as_ref()
            .map(rfd::FileHandle::path)
            .map(Path::to_owned)
            .ok_or(Error::DialogClosed)?
    };
    tokio::fs::write(&path, contents)
        .await
        .map_err(|error| Error::IoError(error.kind()))?;

    Ok(path)
}
