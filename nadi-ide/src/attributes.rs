use crate::icons;
use iced::widget::{Column, column, horizontal_space, row, scrollable, svg, text, toggler};
use iced::{Element, Fill, Task, Theme, color};
use nadi_core::attrs::AttrMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub struct AttrView {
    label: String,
    // attributes: AttrMap,
    names: Vec<String>,
    types: Vec<String>,
    values: Vec<String>,
}

impl AttrView {
    pub fn load_attrs(&mut self, label: String, attrs: &AttrMap) {
        // self.attributes = attrs.clone()
        self.label = label;
        self.names = Vec::with_capacity(attrs.len());
        self.types = Vec::with_capacity(attrs.len());
        self.values = Vec::with_capacity(attrs.len());
        for item in attrs {
            self.names.push(item.0.to_string());
            self.types.push(item.1.type_name().to_string());
            let val = item.1.to_string().replace("\n", "↵");
            if val.len() > 75 {
                self.values.push(format!("{}...", &val[..70]));
            } else {
                self.values.push(val)
            }
        }
    }

    pub fn view(&self) -> Element<'_, ()> {
        let mut controls = row![text(&self.label), horizontal_space()]
            .spacing(10)
            .padding(10);
        let names = Column::from_vec(
            self.names
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    text(s)
                        // will work on 0.14
                        // https://github.com/iced-rs/iced/pull/2723
                        .wrapping(iced::widget::text::Wrapping::None)
                        .color(odd_even_color(i))
                        .into()
                })
                .collect(),
        );
        let types = Column::from_vec(
            self.types
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    text(s)
                        .wrapping(iced::widget::text::Wrapping::None)
                        .color(odd_even_color(i))
                        .into()
                })
                .collect(),
        );
        let values = Column::from_vec(
            self.values
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    text(s)
                        .wrapping(iced::widget::text::Wrapping::None)
                        .color(odd_even_color(i))
                        .into()
                })
                .collect(),
        );
        column![
            controls,
            scrollable(row![names, types, values].spacing(10.0)).width(Fill)
        ]
        .padding(10)
        .into()
    }
}

fn odd_even_color(ind: usize) -> iced::Color {
    if (ind % 2) == 0 {
        color!(0x444444)
    } else {
        color!(0xaaaaaa)
    }
}
