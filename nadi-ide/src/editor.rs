use crate::icons;
use iced::highlighter;
use iced::time::{self, Duration, Instant};
use iced::widget::{
    column, container, horizontal_space, pick_list, row, scrollable, stack, text,
    text::{Rich, Span},
    text_editor, vertical_rule,
};
use iced::{Element, Fill, Font, Subscription, Task, Theme};
use nadi_core::{
    functions::{FuncArg, FuncArgType},
    parser::{
        ParseError,
        highlight::NadiFileType,
        tokenizer::{self, TaskToken},
    },
    tasks::FunctionType,
};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
pub mod colors;
pub mod my_hl;

#[derive(Clone, Debug)]
pub struct EditorFunction {
    pub ty: FunctionType,
    /// actual Function Type (could be environment function)
    pub ity: FunctionType,
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
            Span::new(self.ity.name()).color(colors::ARG_COLOR_FUNCTY),
            Span::new(" "),
            Span::new(&self.name),
            Span::new("(").color(colors::ARG_COLOR_SYM),
        ];
        if let Some(last) = args.pop() {
            for txt in args {
                texts.extend(txt);
                texts.push(Span::new(", ").color(colors::ARG_COLOR_SYM));
            }
            texts.extend(last);
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
    content_hist: Vec<String>,
    content_index: usize,
    is_hist_dirty: bool,
    last_edit: Instant,
    error: Option<ParseError>,
    embedded: bool,
}

static EDITOR_DEFAULT: &str = r#"# example nadi script
# you can load network with a string
network load_str("a -> b\n b -> d\n c -> d");

# if you have a file with network information uncomment
# and use the code below, for windows, careful to use \\
# instead of \ as it has special meaning inside strings

# network load_file("path/to/file.network");

# you can change visual properties of the nodes
node.visual.textcolor = "red"
# as node functions are run at each node you can set them individually
node[a,c].visual.nodecolor = "green";
node(INDEX < 2).visual.nodesize = 8;
node[b].visual.linewidth = 4;
node[d].visual.linecolor = "red";
# try different shapes
node.visual.nodeshape = "ellipse:0.5";
node.visual.nodeshape = "circle";
node.visual.nodeshape = "rectangle:0.5";
node.visual.nodeshape = "box";
node.visual.nodeshape = "triangle";
node.visual.nodeshape = "triangle:2";

# Use the bottons on the top to open file and start coding;
# Hope Nadi is useful for your research/learning experience :)
"#;

impl Default for Editor {
    fn default() -> Self {
        // since the content default text is "\n"
        let mut content = text_editor::Content::new();
        let content_hist = vec![content.text()];
        let content_index = content_hist.len();
        content = text_editor::Content::with_text(EDITOR_DEFAULT);
        Self {
            theme: highlighter::Theme::SolarizedDark,
            curr_func: None,
            status: String::new(),
            file: None,
            is_dirty: false,
            is_loading: false,
            content,
            content_hist,
            content_index,
            is_hist_dirty: false,
            last_edit: Instant::now(),
            error: None,
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
    ToggleComment,
    UndoEdit,
    RedoEdit,
    MaybeSaveEditHist,
    SaveEditHist,
    ResetEditHist,
    MaybeParseTasks,
    FunctionAtMark(Option<(FunctionType, String)>),
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
            Message::FunctionAtMark(func) => {
                let (ty, name) = match func {
                    Some(t) => t,
                    None => {
                        self.curr_func = None;
                        self.status = "".to_string();
                        return Task::none();
                    }
                };
                if self.embedded {
                    if let Some(curr) = &self.curr_func {
                        if curr.ty == ty && curr.name == name {
                            return Task::none();
                        }
                    }
                    Task::perform(async { (ty, name) }, Message::FuncSignature)
                } else {
                    Task::none()
                }
            }
            Message::EditorAction(action) => {
                if action.is_edit() {
                    self.is_dirty = true;
                    self.is_hist_dirty = true;
                    self.last_edit = Instant::now();
                    self.error = None;
                }
                self.content.perform(action);
                Task::perform(
                    func_at_mark(self.content.text(), self.content.cursor_position()),
                    Message::FunctionAtMark,
                )
            }
            Message::NewFile => {
                if !self.is_loading {
                    self.file = None;
                    self.content = text_editor::Content::new();
                }
                Task::done(Message::ResetEditHist)
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
                Task::done(Message::ResetEditHist)
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
                Task::done(Message::SaveEditHist)
            }
            Message::ToggleComment => {
                if let Some(sel) = self.content.selection() {
                    let new_sel = toggle_comment(&sel);

                    self.content
                        .perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                    self.content
                        .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                            Arc::new(new_sel),
                        )));
                } else {
                    self.content
                        .perform(text_editor::Action::Edit(text_editor::Edit::Insert('#')));
                    self.content
                        .perform(text_editor::Action::Edit(text_editor::Edit::Insert(' ')));
                }
                Task::done(Message::SaveEditHist)
            }
            Message::UndoEdit => {
                let cont = self.content.text();
                if self.content_index == self.content_hist.len() {
                    // if this is the first undo and last hist is
                    // something else, save it
                    if cont != self.content_hist[self.content_index - 1] {
                        self.content_hist.push(cont);
                        self.content_index += 1;
                    }
                }
                if let Some(c) = self.content_hist.get(self.content_index - 2) {
                    self.content = text_editor::Content::with_text(c);
                    self.content_index -= 1;
                }
                self.is_hist_dirty = false;
                Task::none()
            }
            Message::RedoEdit => {
                if let Some(c) = self.content_hist.get(self.content_index) {
                    self.content = text_editor::Content::with_text(c);
                    self.content_index += 1;
                }
                self.is_hist_dirty = false;
                Task::none()
            }
            Message::MaybeSaveEditHist => {
                if self.is_hist_dirty {
                    let lag = Instant::now().duration_since(self.last_edit).as_millis();
                    // saving after 0.3 second of last edit action
                    if lag > 300 {
                        let cont = self.content.text();
                        if Some(&cont) != self.content_hist.get(self.content_index - 1) {
                            return Task::done(Message::SaveEditHist);
                        }
                    }
                }
                Task::none()
            }
            Message::SaveEditHist => {
                let cont = self.content.text();
                self.content_hist.truncate(self.content_index);
                self.content_hist.push(cont);
                self.content_index = self.content_hist.len();
                self.is_hist_dirty = false;
                Task::none()
            }
            Message::ResetEditHist => {
                let cont = self.content.text();
                self.content_hist = vec![cont];
                self.content_index = 1;
                self.is_hist_dirty = false;
                Task::none()
            }
            Message::MaybeParseTasks => {
                let lag = Instant::now().duration_since(self.last_edit).as_secs();
                if lag > 2 {
                    let cont = self.content.text();
                    self.error = nadi_core::parser::tasks::parse(
                        nadi_core::parser::tokenizer::get_tokens(&cont),
                    )
                    .err();
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
            icons::danger_action(
                icons::trash_icon(),
                "New (Ctrl + n)",
                Some(Message::NewFile)
            ),
            icons::action(
                icons::open_icon(),
                "Open (Ctrl + o)",
                Some(Message::OpenFile)
            ),
            icons::action(
                icons::download_icon(),
                "Save (Ctrl + s)",
                self.is_dirty.then_some(Message::SaveFile)
            ),
            icons::action(
                icons::comment_icon(),
                "Toggle Comment (Alt + ;)",
                Some(Message::ToggleComment)
            ),
            icons::action(
                icons::left_icon(),
                "Undo (Ctrl + z)",
                (self.content_index > 0).then(|| Message::UndoEdit)
            ),
            icons::action(
                icons::right_icon(),
                "Redo (Ctrl + y)",
                (self.content_index < self.content_hist.len()).then(|| Message::RedoEdit)
            ),
        ];
        if self.embedded {
            controls = controls
                .push(vertical_rule(1.0))
                .push(icons::action(
                    icons::run_all_icon(),
                    "Run Selection/Line (Ctrl + Enter)",
                    Some(Message::RunTask),
                ))
                .push(icons::action(
                    icons::terminal_icon(),
                    "Run Buffer (Ctrl + Shift + Enter)",
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
            .key_binding(key_binding)
            .font(Font::MONOSPACE);
        let ext = self
            .file
            .as_deref()
            .and_then(Path::extension)
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("tasks");
        let mut editor: Element<_> = match NadiFileType::from_str(ext) {
            // use custom highlights for nadi files
            Ok(nft) => editor
                .highlight_with::<my_hl::NadiHighlighter>((nft, 0), my_hl::hlto_format)
                .into(),
            _ => editor.highlight(ext, self.theme).into(),
        };
        if let Some(e) = &self.error {
            editor = stack([
                editor,
                container(
                    text(e.user_msg(None))
                        .style(text::danger)
                        .font(Font::MONOSPACE)
                        .width(Fill)
                        .align_x(iced::alignment::Horizontal::Right),
                )
                .align_x(iced::alignment::Horizontal::Right)
                .style(parse_error_overlay)
                .padding(10)
                .into(),
            ])
            .into();
        }
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

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            time::every(Duration::from_millis(100)).map(|_| Message::MaybeSaveEditHist),
            time::every(Duration::from_secs(1)).map(|_| Message::MaybeParseTasks),
        ])
    }
}

fn key_binding(kp: text_editor::KeyPress) -> Option<text_editor::Binding<Message>> {
    if kp.status != text_editor::Status::Focused {
        return text_editor::Binding::<Message>::from_key_press(kp);
    }

    use iced::keyboard::{Key, key::Named};
    match kp.key.as_ref() {
        Key::Character(";") if kp.modifiers.alt() => {
            return Some(text_editor::Binding::Custom(Message::ToggleComment));
        }
        Key::Named(Named::Enter) if kp.modifiers.control() && kp.modifiers.shift() => {
            return Some(text_editor::Binding::Custom(Message::RunAllTask));
        }
        Key::Named(Named::Enter) if kp.modifiers.control() => {
            return Some(text_editor::Binding::Custom(Message::RunTask));
        }
        Key::Character("s") if kp.modifiers.control() => {
            return Some(text_editor::Binding::Custom(Message::SaveFile));
        }
        Key::Character("o") if kp.modifiers.control() => {
            return Some(text_editor::Binding::Custom(Message::OpenFile));
        }
        Key::Character("n") if kp.modifiers.control() => {
            return Some(text_editor::Binding::Custom(Message::NewFile));
        }
        Key::Character("z") if kp.modifiers.control() => {
            return Some(text_editor::Binding::Custom(Message::UndoEdit));
        }
        Key::Character("y") if kp.modifiers.control() => {
            return Some(text_editor::Binding::Custom(Message::RedoEdit));
        }
        _ => (),
    }
    match kp.text.as_ref().map(|s| s.as_str()) {
        // some basic autopairing for symbols
        Some("(") => {
            return Some(text_editor::Binding::Sequence(vec![
                text_editor::Binding::Insert('('),
                text_editor::Binding::Insert(')'),
                text_editor::Binding::Move(text_editor::Motion::Left),
            ]));
        }
        Some("[") => {
            return Some(text_editor::Binding::Sequence(vec![
                text_editor::Binding::Insert('['),
                text_editor::Binding::Insert(']'),
                text_editor::Binding::Move(text_editor::Motion::Left),
            ]));
        }
        Some("{") => {
            return Some(text_editor::Binding::Sequence(vec![
                text_editor::Binding::Insert('{'),
                text_editor::Binding::Insert('}'),
                text_editor::Binding::Move(text_editor::Motion::Left),
            ]));
        }
        Some("\"") => {
            return Some(text_editor::Binding::Sequence(vec![
                text_editor::Binding::Insert('"'),
                text_editor::Binding::Insert('"'),
                text_editor::Binding::Move(text_editor::Motion::Left),
            ]));
        }
        _ => (),
    }
    text_editor::Binding::<Message>::from_key_press(kp)
}

#[derive(Debug, Clone)]
pub enum Error {
    DialogClosed,
    IoError(std::io::ErrorKind),
}

async fn open_file() -> Result<(PathBuf, Arc<String>), Error> {
    let picked_file = rfd::AsyncFileDialog::new()
        .set_title("Open a text file...")
        .add_filter(
            "Recommended Files",
            &[
                "net", "network", "tasks", "toml", "txt", "md", "py", "rs", "r",
            ],
        )
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

async fn func_at_mark(text: String, mark: (usize, usize)) -> Option<(FunctionType, String)> {
    let line = mark.0;
    // if the current line can be parsed into a proper task, use that
    let task_str = text.lines().nth(line)?;
    let tokens_v = tokenizer::get_tokens(task_str);
    let mut tokens = tokens_v.iter().peekable();
    let mut ty = None;
    let mut name = None;
    let mut col = 0;
    while col < mark.1 {
        let tk = match tokens.next() {
            Some(t) => t,
            None => break,
        };
        col += tk.content.len();

        match &tk.ty {
            TaskToken::Function => {
                name = Some(tk.content.to_string());
            }
            TaskToken::Keyword(kw) if ty.is_none() => {
                ty = FunctionType::from_keyword(kw);
            }
            _ => (),
        }
    }
    ty.and_then(|t| name.map(|n| (t, n)))
}

fn toggle_comment(selection: &str) -> String {
    let iscomment = selection
        .lines()
        .map(|l| l.trim())
        .all(|l| l.is_empty() || l.starts_with('#'));
    let mut newlines =
        String::with_capacity(selection.len() + iscomment as usize * selection.lines().count() * 2);
    if iscomment {
        for l in selection.lines() {
            if !l.trim().is_empty() {
                let (x, y) = l.split_once('#').expect("should have #");
                newlines.push_str(x);
                let y = y.strip_prefix(' ').unwrap_or(y);
                newlines.push_str(y);
            }
            newlines.push('\n');
        }
    } else {
        for l in selection.lines() {
            if !l.trim().is_empty() {
                newlines.push('#');
                newlines.push(' ');
                newlines.push_str(l);
            }
            newlines.push('\n');
        }
    }
    if !selection.ends_with('\n') {
        // remove the extra '\n' if not in selection as
        // lines() ignores the last '\n'
        newlines.pop();
    }
    newlines
}

fn parse_error_overlay(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    let bg: iced::Background = palette.background.weak.color.into();
    container::Style {
        background: Some(bg.scale_alpha(0.25)),
        border: iced::border::rounded(2),
        ..container::Style::default()
    }
}
