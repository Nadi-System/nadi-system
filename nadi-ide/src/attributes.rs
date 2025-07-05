use crate::icons;
use iced::widget::{Column, column, container, horizontal_space, row, scrollable, text};
use iced::{Element, Fill, Length, color};
use nadi_core::attrs::{AttrMap, Attribute};

/// State of Attribute Viewer
#[derive(Default)]
pub struct AttrView {
    label: String,
    // attributes: AttrMap,
    values: Vec<Attr>,
}

#[derive(Clone, Debug)]
pub enum Message {
    Toggle(String),
    CollapseAll,
    ExpandAll,
}

/// Each attribute in the attribute viewer
struct Attr {
    id: String,
    name: String,
    ty: String,
    val: AttrVal,
    expanded: bool,
}

/// Different type of values for Attribute
enum AttrVal {
    Single(String),
    Multiple(Vec<Attr>),
}

impl Attr {
    fn new(attr: &Attribute, name: &str, pre: &str) -> Attr {
        let id = if pre.is_empty() {
            name.to_string()
        } else {
            format!("{pre}.{name}")
        };
        let (expanded, ty, val) = match attr {
            Attribute::Array(ar) => (
                ar.len() < 5,
                format!("Array [{}]", ar.len()),
                AttrVal::Multiple(
                    ar.iter()
                        .enumerate()
                        .map(|(i, a)| Attr::new(a, &i.to_string(), &id))
                        .collect(),
                ),
            ),
            Attribute::Table(am) => (
                am.len() < 5,
                format!("Table [{}]", am.len()),
                AttrVal::Multiple(am.iter().map(|a| Attr::new(a.1, a.0, &id)).collect()),
            ),
            a => (
                true,
                a.type_name().to_string(),
                AttrVal::Single(a.to_string()),
            ),
        };
        Attr {
            id: id.clone(),
            name: name.to_string(),
            ty,
            val,
            expanded,
        }
    }

    fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }

    fn set_expanded(&mut self, exp: bool) {
        self.expanded = exp;
        let mem = match self.val {
            AttrVal::Single(_) => return,
            AttrVal::Multiple(ref mut ar) => ar,
        };

        for i in mem {
            i.set_expanded(exp);
        }
    }

    fn toggle_internal(&mut self, id: &str) {
        let mem = match self.val {
            AttrVal::Single(_) => return,
            AttrVal::Multiple(ref mut ar) => ar,
        };

        for i in mem {
            if id.starts_with(&i.id) {
                if id.len() > i.id.len() {
                    i.toggle_internal(&id);
                } else {
                    i.toggle();
                }
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let header = container(
            row![
                text(&self.name).width(Length::FillPortion(3)),
                text(&self.ty)
                    .width(Length::FillPortion(2))
                    .align_x(iced::alignment::Horizontal::Right),
                if self.expanded {
                    icons::action(
                        icons::resize_small_icon(),
                        "Collapse",
                        Some(Message::Toggle(self.id.clone())),
                    )
                } else {
                    icons::action(
                        icons::resize_full_icon(),
                        "Expand",
                        Some(Message::Toggle(self.id.clone())),
                    )
                }
            ]
            .spacing(4)
            .padding(5),
        )
        .width(Fill)
        .style(tab_title);
        if self.expanded {
            let val: Element<_> = match &self.val {
                AttrVal::Single(v) => text(v).into(),
                AttrVal::Multiple(attrs) => {
                    let rows: Vec<Element<_>> = attrs.iter().map(|a| a.view()).collect();
                    Column::from_vec(rows)
                        .padding(iced::padding::Padding {
                            left: 12.0,
                            top: 4.0,
                            bottom: 4.0,
                            right: 4.0,
                        })
                        .spacing(10.0)
                        .width(Fill)
                        .into()
                }
            };
            container(column![header, val])
        } else {
            container(column![header])
        }
        .style(tab_contents)
        .into()
    }
}

impl AttrView {
    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::Toggle(id) => {
                for i in &mut self.values {
                    if id.starts_with(&i.id) {
                        if id.len() > i.id.len() {
                            i.toggle_internal(&id);
                        } else {
                            i.toggle();
                        }
                    }
                }
            }
            Message::CollapseAll => {
                for i in &mut self.values {
                    i.set_expanded(false);
                }
            }
            Message::ExpandAll => {
                for i in &mut self.values {
                    i.set_expanded(true);
                }
            }
        }
    }
    /// Loads the attribute values from a [`AttrMap`]
    pub fn load_attrs(&mut self, label: String, attrs: &AttrMap) {
        // self.attributes = attrs.clone()
        self.label = label;
        self.values = attrs
            .iter()
            .map(|item| Attr::new(item.1, item.0, ""))
            .collect();
        self.values.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Generate view for Iced Window
    pub fn view(&self) -> Element<'_, Message> {
        let controls = row![
            text(&self.label),
            horizontal_space(),
            icons::action(
                icons::resize_small_icon(),
                "Collapse All",
                Some(Message::CollapseAll),
            ),
            icons::action(
                icons::resize_full_icon(),
                "Expand All",
                Some(Message::ExpandAll),
            )
        ]
        .spacing(10)
        .padding(10);
        let rows: Vec<Element<_>> = self.values.iter().map(|v| v.view()).collect();
        column![
            controls,
            scrollable(Column::from_vec(rows).spacing(10.0).width(Fill))
                .spacing(10)
                .width(Fill)
        ]
        .padding(10)
        .width(Fill)
        .into()
    }
}

fn tab_title(theme: &iced::Theme) -> container::Style {
    let mut style = container::Style {
        background: Some(iced::Background::Color(
            if theme.extended_palette().is_dark {
                color!(0xaaaaaa)
            } else {
                color!(0x444444)
            }
            .scale_alpha(0.3),
        )),
        ..Default::default()
    };
    style.border.radius = iced::border::Radius::new(0).top(5);
    style
}

fn tab_contents(theme: &iced::Theme) -> container::Style {
    let mut style = container::Style {
        background: Some(iced::Background::Color(
            if theme.extended_palette().is_dark {
                color!(0x444444)
            } else {
                color!(0xaaaaaa)
            }
            .scale_alpha(0.3),
        )),
        ..Default::default()
    };
    style.border.radius = iced::border::Radius::new(5);
    style
}
