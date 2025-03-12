use iced::widget::{button, column, horizontal_space, markdown, row, scrollable, toggler};
use iced::{widget::Column, Color, Element, Length, Theme};
use nadi_core::functions::{FuncArg, NadiFunctions};

static FUNC_WIDTH: f32 = 300.0;

pub fn main() -> iced::Result {
    iced::application("NADI Help", MdHelp::update, MdHelp::view)
        .theme(MdHelp::theme)
        .run()
}

#[derive(Clone, Debug)]
enum State {
    Node,
    Network,
    Env,
}

struct MdHelp {
    light_theme: bool,
    functions: NadiFunctions,
    state: State,
    markdown: Vec<markdown::Item>,
}

#[derive(Debug, Clone)]
enum Message {
    LinkClicked(markdown::Url),
    Function(State, String),
    StateChange(State),
    ThemeChange(bool),
}

impl Default for MdHelp {
    fn default() -> Self {
        Self {
            light_theme: false,
            functions: NadiFunctions::new(),
            state: State::Node,
            markdown: markdown::parse("**Click** on a function to see the help!").collect(),
        }
    }
}

// Macro instead of function as func are different types, but the
// traits have same functions
macro_rules! help {
    ($ty:literal, $name:expr, $func:expr) => {
        help_to_markdown(
            $ty,
            &$name,
            &$func.args(),
            &$func.short_help(),
            &$func.help(),
            &$func.code(),
        )
    };
}

impl MdHelp {
    fn view(&self) -> Column<'_, Message> {
        let controls = row![
            button("Env")
                .on_press(Message::StateChange(State::Env))
                .style(match self.state {
                    State::Env => button::success,
                    _ => button::primary,
                }),
            button("Node")
                .on_press(Message::StateChange(State::Node))
                .style(match self.state {
                    State::Node => button::success,
                    _ => button::primary,
                }),
            button("Network")
                .on_press(Message::StateChange(State::Network))
                .style(match self.state {
                    State::Network => button::success,
                    _ => button::primary,
                }),
            horizontal_space(),
            toggler(self.light_theme)
                .label(if self.light_theme { "Light" } else { "Dark" })
                .on_toggle(Message::ThemeChange)
        ]
        .spacing(20)
        .padding(10);
        let md = markdown::view(
            &self.markdown,
            markdown::Settings::default(),
            md_style(self.light_theme),
        )
        .map(Message::LinkClicked);
        let funcs: Vec<Element<_>> = list_functions(&self.functions, &self.state)
            .into_iter()
            .enumerate()
            .map(|(i, n)| {
                button(n)
                    .on_press(Message::Function(self.state.clone(), n.to_string()))
                    .width(Length::Fill)
                    .style(if (i % 2) == 0 {
                        secondary_even
                    } else {
                        secondary_odd
                    })
                    .into()
            })
            .collect();

        let list = Column::from_vec(funcs).width(FUNC_WIDTH);
        let main = row![scrollable(list), scrollable(md)]
            .spacing(10)
            .padding(10);
        column![controls, main].spacing(10)
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::LinkClicked(url) => {
                println!("The following url was clicked: {url}");
            }
            Message::Function(State::Node, func) => {
                if let Some(f) = self.functions.node(&func) {
                    self.markdown = help!("node", func, f);
                }
            }
            Message::Function(State::Network, func) => {
                if let Some(f) = self.functions.network(&func) {
                    self.markdown = help!("network", func, f);
                }
            }
            Message::Function(State::Env, func) => {
                if let Some(f) = self.functions.env(&func) {
                    self.markdown = help!("env", func, f);
                }
            }
            Message::StateChange(state) => {
                self.state = state;
            }
            Message::ThemeChange(t) => {
                self.light_theme = t;
            }
        }
    }

    fn theme(&self) -> Theme {
        if self.light_theme {
            Theme::Light
        } else {
            Theme::Dark
        }
    }
}

fn list_functions<'a>(functions: &'a NadiFunctions, state: &State) -> Vec<&'a str> {
    let mut func: Vec<&str> = match state {
        State::Node => functions
            .node_functions()
            .iter()
            .map(|n| n.0.as_str())
            .collect(),
        State::Network => functions
            .network_functions()
            .iter()
            .map(|n| n.0.as_str())
            .collect(),
        State::Env => functions
            .env_functions()
            .iter()
            .map(|n| n.0.as_str())
            .collect(),
    };
    func.sort();
    func
}

fn help_to_markdown(
    ty: &str,
    name: &str,
    args: &[FuncArg],
    short: &str,
    long: &str,
    code: &str,
) -> Vec<markdown::Item> {
    let mut items = vec![];
    let sig = args
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<String>>()
        .join(", ");
    items.push(format!(
        "# {ty} <span color=\"blue\">{name}</span>\n```python\n{ty} {name}({sig})\n```\n\n{short}"
    ));
    items.push("## Arguments".to_string());
    args.iter()
        .for_each(|f| items.push(format!("- `{}` => {}", f.to_string(), f.help)));
    items.push("\n".to_string());
    items.push(long[short.len()..].trim().to_string());
    items.push(format!("# Code\n```rust\n{code}\n```\n"));
    markdown::parse(&items.join("\n")).collect()
}

fn md_style(light: bool) -> markdown::Style {
    let pc = if light { 0.0 } else { 1.0 };
    let inline_code_highlight = markdown::Highlight {
        background: iced::Background::Color(Color {
            r: 0.5,
            g: 0.5,
            b: 0.5,
            a: 0.5,
        }),
        border: iced::Border {
            color: Color {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 0.0,
            },
            width: 1.0,
            radius: iced::border::Radius::from(5.0),
        },
    };
    let inline_code_padding = iced::Padding::from(2.0);
    let inline_code_color = Color {
        r: pc,
        g: pc,
        b: pc,
        a: 1.0,
    };
    let link_color = Color {
        r: 0.5,
        g: 0.5,
        b: 1.0,
        a: 1.0,
    };

    markdown::Style {
        inline_code_highlight,
        inline_code_padding,
        inline_code_color,
        link_color,
    }
}

fn secondary_odd(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let pair = palette.secondary.base;
    let base = button::Style {
        background: Some(iced::Background::Color(pair.color)),
        text_color: pair.text,
        border: iced::border::rounded(0),
        ..button::Style::default()
    };

    match status {
        button::Status::Active | button::Status::Pressed => base,
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(palette.secondary.strong.color)),
            ..base
        },
        button::Status::Disabled => base,
    }
}

fn secondary_even(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let pair = palette.secondary.base;
    let base = button::Style {
        background: Some(iced::Background::Color(pair.color.scale_alpha(0.5))),
        text_color: pair.text,
        border: iced::border::rounded(0),
        ..button::Style::default()
    };

    match status {
        button::Status::Active | button::Status::Pressed => base,
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(palette.secondary.strong.color)),
            ..base
        },
        button::Status::Disabled => base,
    }
}
