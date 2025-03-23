use iced::widget::{Column, column, container, horizontal_space, row, scrollable, text};
use iced::{Element, Fill, Length, color};
use nadi_core::attrs::AttrMap;

#[derive(Default)]
pub struct AttrView {
    label: String,
    // attributes: AttrMap,
    values: Vec<(String, String, String)>,
}

impl AttrView {
    pub fn load_attrs(&mut self, label: String, attrs: &AttrMap) {
        // self.attributes = attrs.clone()
        self.label = label;
        self.values = attrs
            .iter()
            .map(|item| {
                (
                    item.0.to_string(),
                    item.1.type_name().to_string(),
                    item.1.to_string(),
                )
            })
            .collect();
    }

    pub fn view(&self) -> Element<'_, ()> {
        let controls = row![text(&self.label), horizontal_space()]
            .spacing(10)
            .padding(10);
        let rows: Vec<Element<_>> = self
            .values
            .iter()
            .map(|(name, ty, val)| {
                container(column![
                    container(
                        row![
                            text(name).width(Length::FillPortion(3)),
                            text(ty)
                                .width(Length::FillPortion(2))
                                .align_x(iced::alignment::Horizontal::Right)
                        ]
                        .padding(5)
                    )
                    .width(Fill)
                    .style(tab_title),
                    container(text(val)).padding(5)
                ])
                .style(tab_contents)
                .into()
            })
            .collect();
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
    let mut style = container::Style::default();
    style.background = Some(iced::Background::Color(
        if theme.extended_palette().is_dark {
            color!(0xaaaaaa)
        } else {
            color!(0x444444)
        }
        .scale_alpha(0.3),
    ));
    style.border.radius = iced::border::Radius::new(0).top(5);
    style
}

fn tab_contents(theme: &iced::Theme) -> container::Style {
    let mut style = container::Style::default();
    style.background = Some(iced::Background::Color(
        if theme.extended_palette().is_dark {
            color!(0x444444)
        } else {
            color!(0xaaaaaa)
        }
        .scale_alpha(0.3),
    ));
    style.border.radius = iced::border::Radius::new(5);
    style
}
