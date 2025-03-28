use crate::help::FuncType;
use crate::icons;
use iced::highlighter;
use iced::widget::{column, horizontal_space, pick_list, row, text, text_editor, vertical_rule};
use iced::{Element, Fill, Font, Task, Theme};
use nadi_core::{
    parser::tasks,
    parser::tokenizer::{self, TaskToken},
    tasks::{TaskInput, TaskKeyword, TaskType},
};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
pub mod my_hl;

pub struct Editor {
    theme: highlighter::Theme,
    pub function: Option<(FuncType, String)>,
    signature: String,
    file: Option<PathBuf>,
    is_dirty: bool,
    is_loading: bool,
    pub content: text_editor::Content,
    embedded: bool,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            theme: highlighter::Theme::SolarizedDark,
            function: None,
            signature: String::new(),
            file: None,
            is_dirty: false,
            is_loading: false,
            content: text_editor::Content::default(),
            embedded: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    EditorAction(text_editor::Action),
    ThemeChange(highlighter::Theme),
    NewFile,
    OpenFile,
    FileOpened(Result<(PathBuf, Arc<String>), Error>),
    SaveFile,
    FileSaved(Result<PathBuf, Error>),
    Comment,
    FuncAtMark(Option<(FuncType, String)>),
    // these messages are only sent when embedded; and are handled in
    // the main window
    RunAllTask,
    RunTask,
    SearchHelp,
    HelpTask,
}

impl Editor {
    pub fn embed(mut self) -> Self {
        self.embedded = true;
        self
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChange(theme) => {
                self.theme = theme;
                Task::none()
            }
            Message::FuncAtMark(func) => {
                // todo get signature from the actual function
                self.signature = func
                    .as_ref()
                    .map(|(t, n)| format!("{t} {n}"))
                    .unwrap_or_default();
                self.function = func;
                Task::none()
            }
            Message::EditorAction(action) => {
                self.is_dirty = self.is_dirty || action.is_edit();
                self.content.perform(action);
                Task::perform(
                    task_at_mark(self.content.text(), self.content.cursor_position()),
                    Message::FuncAtMark,
                )
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
            // remaining ones should be handled in main window, and
            // should be absent during non embed status; type system
            // can't help here, so be careful
            _ => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut controls = row![
            icons::danger_action(icons::trash_icon(), "New", Some(Message::NewFile)),
            icons::action(icons::open_icon(), "Open", Some(Message::OpenFile)),
            icons::action(
                icons::download_icon(),
                "Save",
                self.is_dirty.then_some(Message::SaveFile)
            ),
            icons::action(icons::comment_icon(), "Comment", Some(Message::Comment)),
        ];
        if self.embedded {
            controls = controls
                .push(vertical_rule(1.0))
                .push(icons::action(
                    icons::run_all_icon(),
                    "Run Selection/Line",
                    Some(Message::RunTask),
                ))
                .push(icons::action(
                    icons::terminal_icon(),
                    "Run Buffer",
                    Some(Message::RunAllTask),
                ))
                .push(icons::action(
                    icons::search_icon(),
                    "Search in Help Window",
                    Some(Message::SearchHelp),
                ))
                .push(icons::action(
                    icons::help_icon(),
                    "Help",
                    self.function.as_ref().map(|_| Message::HelpTask),
                ));
        }
        controls = controls.push(horizontal_space());
        controls = controls.push(pick_list(
            highlighter::Theme::ALL,
            Some(self.theme),
            Message::ThemeChange,
        ));

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
        let editor = text_editor(&self.content)
            .height(Fill)
            .on_action(Message::EditorAction)
            .font(Font::MONOSPACE);
        let ext = self
            .file
            .as_deref()
            .and_then(Path::extension)
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("txt");
        let editor: Element<_> = match my_hl::NadiFileType::from_str(ext) {
            // use custom highlights for nadi files
            Ok(nft) => editor
                .highlight_with::<my_hl::NadiHighlighter>(nft, my_hl::Highlight::to_format)
                .into(),
            _ => editor.highlight(ext, self.theme).into(),
        };
        column![controls.spacing(10).height(30.0), signature, editor, status]
            .padding(10)
            .into()
    }

    pub fn theme(&self) -> Theme {
        if self.theme.is_dark() {
            Theme::Dark
        } else {
            Theme::Light
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

enum State {
    None,
    Kw(TaskKeyword),
}

async fn task_at_mark(text: String, mark: (usize, usize)) -> Option<(FuncType, String)> {
    let line = mark.0;
    // if the current line can be parsed into a proper task, use that
    let task_str = text.lines().nth(line)?;
    let tokens = tokenizer::get_tokens(task_str).ok()?;
    if let Ok([task, ..]) = tasks::parse(tokens).as_deref() {
        return if let TaskInput::Function(fc) = &task.input {
            let fty = match task.ty {
                TaskType::Node(_) => FuncType::Node,
                TaskType::Network(_) => FuncType::Network,
                TaskType::Env => FuncType::Env,
                _ => return None,
            };
            Some((fty, fc.name.clone()))
        } else {
            None
        };
    }

    // if not parse the whole thing and deduce the last function call
    let mut state = State::None;
    let mut func = None;
    let mut tokens = tokenizer::VecTokens::new(tokenizer::get_tokens(&text).ok()?);
    while tokens.line <= line {
        match tokens.next_no_ws(true) {
            Some(t) => match t.ty {
                TaskToken::Keyword(k) => {
                    state = State::Kw(k);
                }
                TaskToken::Function => {
                    if let State::Kw(kw) = state {
                        func = Some((kw, t.content.to_string()));
                    }
                    state = State::None;
                }
                _ => (),
            },
            None => break,
        }
    }
    match func {
        Some((kw, func)) => match kw {
            TaskKeyword::Node => Some((FuncType::Node, func)),
            TaskKeyword::Network => Some((FuncType::Network, func)),
            TaskKeyword::Env => Some((FuncType::Env, func)),
            _ => None,
        },
        _ => None,
    }
}
