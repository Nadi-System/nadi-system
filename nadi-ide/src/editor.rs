use crate::icons;
use iced::highlighter;
use iced::widget::{
    column, container, horizontal_space, pick_list, row, scrollable, text,
    text::{Rich, Span},
    text_editor, vertical_rule,
};
use iced::{Element, Fill, Font, Task, Theme};
use nadi_core::{
    expressions::Expression,
    functions::{FuncArg, FuncArgType},
    parser::{highlight::NadiFileType, tasks, tokenizer},
    tasks::{FunctionType, Task as NadiTask},
};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
pub mod colors;
pub mod my_hl;

#[derive(Clone, Debug)]
pub struct EditorFunction {
    pub ty: FunctionType,
    pub name: String,
    pub args: Vec<FuncArg>,
}

impl EditorFunction {
    fn view(&self) -> Rich<'_, Message> {
        let mut args: Vec<Vec<Span<_>>> = self
            .args
            .iter()
            .map(|a| match &a.category {
                FuncArgType::Arg => {
                    vec![
                        Span::new(a.name.as_str()).color(colors::ARG_COLOR_REQ),
                        Span::new(": ").color(colors::ARG_COLOR_TYPE),
                        Span::new(a.ty.to_string()).color(colors::ARG_COLOR_TYPE),
                    ]
                }
                FuncArgType::OptArg => {
                    vec![
                        Span::new(a.name.as_str()),
                        Span::new(": ").color(colors::ARG_COLOR_TYPE),
                        Span::new(a.ty.to_string()).color(colors::ARG_COLOR_TYPE),
                    ]
                }
                FuncArgType::DefArg(val) => vec![
                    Span::new(a.name.as_str()),
                    Span::new(": ").color(colors::ARG_COLOR_TYPE),
                    Span::new(a.ty.to_string()).color(colors::ARG_COLOR_TYPE),
                    Span::new(" = "),
                    Span::new(val.to_string()).color(colors::ARG_COLOR_VAL),
                ],
                FuncArgType::Args => {
                    vec![
                        Span::new("*").color(colors::ARG_COLOR_GLOB),
                        Span::new(a.name.as_str()),
                    ]
                }
                FuncArgType::KwArgs => {
                    vec![
                        Span::new("**").color(colors::ARG_COLOR_GLOB),
                        Span::new(a.name.as_str()),
                    ]
                }
            })
            .collect();
        let mut texts: Vec<Span<_>> = vec![
            Span::new(&self.name).color(colors::ARG_COLOR_FUNC),
            Span::new("(").color(colors::ARG_COLOR_SYM),
        ];
        match args.pop() {
            Some(last) => {
                for txt in args {
                    texts.extend(txt);
                    texts.push(Span::new(", ").color(colors::ARG_COLOR_SYM));
                }
                texts.extend(last);
            }
            None => (),
        }
        texts.push(Span::new(")").color(colors::ARG_COLOR_SYM));
        Rich::with_spans(texts).font(iced::font::Font::MONOSPACE)
    }
}

pub struct Editor {
    theme: highlighter::Theme,
    status: String,
    pub curr_func: Option<EditorFunction>,
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
            curr_func: None,
            status: String::new(),
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
    TaskAtMark(Option<NadiTask>),
    FuncFound(EditorFunction),
    // these messages are only sent when embedded; and are handled in
    // the main window
    FuncSignature((FunctionType, String)),
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
            Message::FuncFound(f) => {
                self.curr_func = Some(f);
                Task::none()
            }
            Message::TaskAtMark(task) => {
                let task = match task {
                    Some(t) => t,
                    None => {
                        self.curr_func = None;
                        self.status = "".to_string();
                        return Task::none();
                    }
                };
                // todo get status from the actual function
                self.status = match task {
                    NadiTask::Eval(et) => {
                        if let Expression::Function(fc) = et.input {
                            if self.embedded {
                                if let Some(curr) = &self.curr_func {
                                    if curr.ty == et.ty && curr.name == fc.name {
                                        return Task::none();
                                    }
                                }
                                return Task::perform(
                                    async { (et.ty, fc.name) },
                                    Message::FuncSignature,
                                );
                            } else {
                                format!("Set {} attribute from the {} function", et.ty, fc.name)
                            }
                        } else if let Some(_) = et.attr {
                            format!("Set {} attribute from the expression", et.ty)
                        } else {
                            format!("Evaluate {} expression", et.ty)
                        }
                    }
                    NadiTask::Attr(at) => format!("Get {} attribute", at.ty),
                    NadiTask::Help(Some(kw), Some(name)) => {
                        format!("Display help for {} function {name}", kw.to_string())
                    }
                    NadiTask::Help(None, Some(name)) => format!("Display help for function {name}"),
                    NadiTask::Help(Some(kw), None) => {
                        format!("Display help for {}", kw.to_string())
                    }
                    NadiTask::Help(None, None) => "Display help".into(),
                    NadiTask::Exit => "Exit the program".into(),
                };
                Task::none()
            }
            Message::EditorAction(action) => {
                self.is_dirty = self.is_dirty || action.is_edit();
                self.content.perform(action);
                Task::perform(
                    task_at_mark(self.content.text(), self.content.cursor_position()),
                    Message::TaskAtMark,
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
                        if let Some(p) = path.parent() {
                            let _ = std::env::set_current_dir(p);
                        }
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
                    self.curr_func.as_ref().map(|_| Message::HelpTask),
                ));
        }
        controls = controls.push(horizontal_space());
        controls = controls.push(pick_list(
            highlighter::Theme::ALL,
            Some(self.theme),
            Message::ThemeChange,
        ));

        let status: Element<_> = if let Some(f) = &self.curr_func {
            f.view().into()
        } else {
            text(self.status.clone()).into()
        };
        let fileinfo = row![
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
            .unwrap_or("tasks");
        let editor: Element<_> = match NadiFileType::from_str(ext) {
            // use custom highlights for nadi files
            Ok(nft) => editor
                .highlight_with::<my_hl::NadiHighlighter>((nft, 0), my_hl::hlto_format)
                .into(),
            _ => editor.highlight(ext, self.theme).into(),
        };
        column![
            controls.spacing(10).height(30.0),
            scrollable(container(status).padding(5.0))
                .height(30.0)
                .width(Fill),
            editor,
            fileinfo
        ]
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

async fn task_at_mark(text: String, mark: (usize, usize)) -> Option<NadiTask> {
    let line = mark.0;
    // if the current line can be parsed into a proper task, use that
    let task_str = text.lines().nth(line)?;
    let tokens = tokenizer::get_tokens(task_str);
    tasks::parse(tokens).ok()?.get(0).cloned()
}
