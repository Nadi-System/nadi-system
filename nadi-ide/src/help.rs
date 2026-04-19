use crate::editor::colors;
use crate::icons;
use iced::widget::{
    button, center, column, markdown, row, scrollable,
    space::horizontal,
    text,
    text::{Rich, Span},
    text_input, toggler,
};
use iced::{Color, Element, Font, Length, Theme, widget::Column};
use nadi_core::{
    functions::{FuncArg, NadiFunctions},
    tasks::FunctionType,
};

/// Main help to show in the help window
pub static MAIN_HELP: &str = include_str!("../markdown/main.md");

/// Width of the function list sidepane
static FUNC_WIDTH: f32 = 300.0;

pub struct MdHelp {
    pub light_theme: bool,
    functions: NadiFunctions,
    state: Option<FunctionType>,
    search: String,
    markdown: Vec<markdown::Item>,
    collapsed: bool,
    embedded: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    LinkClicked(String),
    Home,
    Github,
    Book,
    ToggleCollapsed,
    Function(FunctionType, String),
    FunctionTypeChange(Option<FunctionType>),
    ThemeChange(bool),
    SearchChange(String),
}

impl Default for MdHelp {
    fn default() -> Self {
        Self::new(None)
    }
}

// Macro instead of function as func are different types, but the
// traits have same functions
macro_rules! help {
    ($ty:expr, $name:expr, $func:expr) => {
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
    pub fn new(functions: Option<NadiFunctions>) -> Self {
        Self {
            light_theme: false,
            functions: functions.unwrap_or_else(NadiFunctions::internals_w_plugins),
            state: None,
            search: String::new(),
            markdown: markdown::parse(MAIN_HELP).collect(),
            collapsed: false,
            embedded: false,
        }
    }
    pub fn embed(mut self) -> Self {
        self.embedded = true;
        self.collapsed = true;
        self
    }
    pub fn view(&self) -> Element<'_, Message> {
        let mut controls = row![
            button("Home").on_press(Message::Home),
            button("Book").on_press(Message::Book),
            button("GitHub").on_press(Message::Github),
            horizontal()
        ]
        .spacing(20)
        .padding(10);
        if !self.embedded {
            controls = controls.push(toggler(self.light_theme).on_toggle(Message::ThemeChange));
        }
        let md = markdown::view(
            &self.markdown,
            markdown::Settings::with_style(md_style(self.light_theme)),
        )
        .map(Message::LinkClicked);

        let toggle_view = button(center(if self.collapsed {
            icons::right_icon()
        } else {
            icons::left_icon()
        }))
        .on_press(Message::ToggleCollapsed)
        .height(Length::Fill)
        .style(button::secondary)
        .width(25);

        let functions: Element<_> = if self.collapsed {
            toggle_view.into()
        } else {
            let ftypes = row![
                button("All")
                    .on_press(Message::FunctionTypeChange(None))
                    .style(match self.state {
                        None => button::success,
                        _ => button::primary,
                    }),
                button("Env")
                    .on_press(Message::FunctionTypeChange(Some(FunctionType::Env)))
                    .style(match self.state {
                        Some(FunctionType::Env) => button::success,
                        _ => button::primary,
                    }),
                button("Node")
                    .on_press(Message::FunctionTypeChange(Some(FunctionType::Node)))
                    .style(match self.state {
                        Some(FunctionType::Node) => button::success,
                        _ => button::primary,
                    }),
                button("Network")
                    .on_press(Message::FunctionTypeChange(Some(FunctionType::Network)))
                    .style(match self.state {
                        Some(FunctionType::Network) => button::success,
                        _ => button::primary,
                    }),
            ]
            .spacing(20)
            .padding(10);
            let funcs: Vec<Element<_>> = list_functions(&self.functions, &self.state, &self.search)
                .into_iter()
                .enumerate()
                .map(|(i, n)| {
                    let label = format!("{}  {}", n.0, n.1);
                    let label: Element<_> = if self.search.trim().is_empty() {
                        text(label).into()
                    } else {
                        function_label(label, &self.search).into()
                    };
                    button(label)
                        .on_press(Message::Function(n.0.clone(), n.1.to_string()))
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
            let search = text_input("Search", &self.search)
                .on_input(Message::SearchChange)
                .padding(10)
                .width(FUNC_WIDTH);
            row![
                column![ftypes, search, scrollable(list)].spacing(10),
                toggle_view
            ]
            .into()
        };

        let main = row![functions, scrollable(md)].spacing(10).padding(10);
        column![controls, main].spacing(10).into()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::LinkClicked(url) => {
                // we can make our own schema for the links to nadi
                // functions later
                let _ = webbrowser::open(&url);
            }
            Message::Home => {
                self.markdown = markdown::parse(MAIN_HELP).collect();
            }
            Message::Book => {
                _ = webbrowser::open("https://nadi-system.github.io/");
            }
            Message::Github => {
                _ = webbrowser::open("https://github.com/Nadi-System/");
            }
            Message::ToggleCollapsed => {
                self.collapsed = !self.collapsed;
            }
            Message::SearchChange(s) => {
                self.search = s;
            }
            Message::Function(FunctionType::Node, func) => {
                if let Some(f) = self.functions.node(&func) {
                    self.markdown = help!("node", func, f);
                }
            }
            Message::Function(FunctionType::Network, func) => {
                if let Some(f) = self.functions.network(&func) {
                    self.markdown = help!("network", func, f);
                }
            }
            Message::Function(FunctionType::Env, func) => {
                if let Some(f) = self.functions.env(&func) {
                    self.markdown = help!("env", func, f);
                }
            }
            Message::FunctionTypeChange(state) => {
                self.state = state;
            }
            Message::ThemeChange(t) => {
                self.light_theme = t;
            }
        }
    }

    pub fn theme(&self) -> Theme {
        if self.light_theme {
            Theme::Light
        } else {
            Theme::Dark
        }
    }
}

enum Term<'a> {
    Search(&'a str),
    Remainder(&'a str),
}

impl<'a> Term<'a> {
    fn split(self, s: &'a str) -> Vec<Term<'a>> {
        match self {
            Self::Remainder(a) => {
                let parts: Vec<_> = a.split(s).map(Self::Remainder).collect();
                let mut joined = Vec::with_capacity(parts.len() * 2);
                for p in parts {
                    joined.push(p);
                    joined.push(Self::Search(s));
                }
                joined.pop();
                joined
            }
            a => vec![a],
        }
    }
}

fn function_label(label: String, search: &str) -> Rich<'_, String, Message> {
    let searches: Vec<&str> = search.trim().split(' ').collect();
    let mut labels = vec![Term::Remainder(&label)];
    for s in searches {
        labels = labels.into_iter().flat_map(|t| t.split(s)).collect();
    }
    let texts: Vec<_> = labels
        .into_iter()
        .map(|t| match t {
            Term::Search(x) => Span::new(x.to_string()).color(colors::ARG_COLOR_FUNCTY),
            Term::Remainder(x) => Span::new(x.to_string()),
        })
        .collect();
    Rich::with_spans(texts)
}

pub fn list_functions<'a>(
    functions: &'a NadiFunctions,
    state: &Option<FunctionType>,
    search: &str,
) -> Vec<(FunctionType, &'a str)> {
    let searches: Vec<&str> = search.trim().split(' ').collect();
    let mut func: Vec<(FunctionType, &str)> = match state {
        Some(FunctionType::Node) => functions
            .node_functions()
            .iter()
            .filter(|n| searches.iter().all(|&s| n.0.contains(s) || s == "node"))
            .map(|n| (FunctionType::Node, n.0.as_str()))
            .collect(),
        Some(FunctionType::Network) => functions
            .network_functions()
            .iter()
            .filter(|n| searches.iter().all(|&s| n.0.contains(s) || s == "network"))
            .map(|n| (FunctionType::Network, n.0.as_str()))
            .collect(),
        Some(FunctionType::Env) => functions
            .env_functions()
            .iter()
            .filter(|n| searches.iter().all(|&s| n.0.contains(s) || s == "env"))
            .map(|n| (FunctionType::Env, n.0.as_str()))
            .collect(),
        None => {
            return vec![
                list_functions(functions, &Some(FunctionType::Env), search),
                list_functions(functions, &Some(FunctionType::Node), search),
                list_functions(functions, &Some(FunctionType::Network), search),
            ]
            .into_iter()
            .flatten()
            .collect();
        }
    };
    func.sort_by(|a, b| a.1.cmp(b.1));
    func
}

pub fn help_to_markdown(
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
        .for_each(|f| items.push(format!("- `{}` => {}", f, f.help)));
    items.push("\n".to_string());
    items.push(long[short.len()..].trim().to_string());
    items.push(format!("# Code\n```rust\n{code}\n```\n"));
    markdown::parse(&items.join("\n")).collect()
}

pub fn md_style(light: bool) -> markdown::Style {
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
    let code_block_font = Font::MONOSPACE;
    let font = Font::DEFAULT;
    let inline_code_font = Font::MONOSPACE;

    markdown::Style {
        inline_code_highlight,
        inline_code_padding,
        inline_code_color,
        link_color,
        code_block_font,
        font,
        inline_code_font,
    }
}

pub fn secondary_odd(theme: &Theme, status: button::Status) -> button::Style {
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

pub fn secondary_even(theme: &Theme, status: button::Status) -> button::Style {
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use std::sync::OnceLock;

    static mut NADI_FUNCS: OnceLock<NadiFunctions> = OnceLock::new();

    #[fixture]
    fn help() -> MdHelp {
        // The static mut ref is for OnceLock, and it is immediately
        // cloned to be used, so it is safe. This just saves us from
        // loading the plugins over and over again for each test,
        // significantly improving the runtime speed.
        #[allow(static_mut_refs)]
        let functions = unsafe { NADI_FUNCS.get_or_init(NadiFunctions::new) }.clone();

        MdHelp {
            functions,
            ..Default::default()
        }
    }

    #[rstest]
    fn test_theme_change(mut help: MdHelp) {
        for b in [true, false, true, false, false, true, true] {
            help.update(Message::ThemeChange(b));
            assert_eq!(help.light_theme, b);
        }
    }

    #[rstest]
    fn test_collapsed_change(mut help: MdHelp) {
        let mut col = help.collapsed;
        for _ in 0..5 {
            help.update(Message::ToggleCollapsed);
            col = !col;
            assert_eq!(help.collapsed, col);
        }
    }

    #[rstest]
    fn test_search_change(mut help: MdHelp) {
        for search in ["test", "testing", "env", "env is"] {
            help.update(Message::SearchChange(search.into()));
            assert_eq!(&help.search, search);
        }
    }

    #[rstest]
    fn test_functiontype_change(mut help: MdHelp) {
        for ft in [
            None,
            Some(FunctionType::Node),
            None,
            Some(FunctionType::Network),
            Some(FunctionType::Node),
            Some(FunctionType::Node),
            Some(FunctionType::Network),
            Some(FunctionType::Env),
        ] {
            help.update(Message::FunctionTypeChange(ft.clone()));
            assert_eq!(help.state, ft);
        }
    }

    // TODO: need to think of ways to make sure the graphics elements are there. Like with searching functions and such.
}
