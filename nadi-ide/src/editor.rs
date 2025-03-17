use iced::widget::{
    button, column, horizontal_space, markdown, row, scrollable, text, text_editor, text_input,
    toggler,
};
use iced::{widget::Column, Color, Element, Fill, Font, Length, Task, Theme};
use nadi_core::functions::{FuncArg, NadiFunctions};
use std::path::PathBuf;

static FUNC_WIDTH: f32 = 300.0;

#[derive(Default)]
pub struct Editor {
    light_theme: bool,
    signature: String,
    file: Option<PathBuf>,
    is_dirty: bool,
    content: text_editor::Content,
    embedded: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    EditorAction(text_editor::Action),
    ThemeChange(bool),
    NewFile,
    OpenFile,
    SaveFile,
}

impl Editor {
    pub fn embed(mut self) -> Self {
        self.embedded = true;
        self
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ThemeChange(theme) => {
                self.light_theme = theme;
            }
            Message::EditorAction(action) => {
                self.is_dirty = self.is_dirty || action.is_edit();
                self.content.perform(action);
            }
            Message::NewFile => (),
            Message::OpenFile => (),
            Message::SaveFile => (),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let controls = row![
            button("New").on_press(Message::NewFile),
            button("Open").on_press(Message::OpenFile),
            button("Save").on_press_maybe(self.is_dirty.then(|| Message::SaveFile)),
            horizontal_space(),
            toggler(self.light_theme)
                .on_toggle_maybe((!self.embedded).then(|| Message::ThemeChange)),
        ]
        .spacing(10)
        .padding(10);

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
                .font(Font::MONOSPACE),
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
