use iced::widget::{
    self, button, center, column, container, horizontal_space, pick_list, row, text, text_editor,
    toggler, tooltip,
};
use iced::{Center, Element, Fill, Font, Task, Theme};

fn icon<'a, Message>(codepoint: char) -> Element<'a, Message> {
    const ICON_FONT: Font = Font::with_name("nadi-icons");

    text(codepoint).font(ICON_FONT).into()
}

pub fn action<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    label: &'a str,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let action = button(center(content).width(20).height(20));

    if let Some(on_press) = on_press {
        tooltip(
            action.on_press(on_press),
            label,
            tooltip::Position::FollowCursor,
        )
        .style(container::rounded_box)
        .into()
    } else {
        action.style(button::secondary).into()
    }
}

pub fn danger_action<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    label: &'a str,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let action = button(center(content).width(20).height(20)).style(button::danger);

    if let Some(on_press) = on_press {
        tooltip(
            action.on_press(on_press),
            label,
            tooltip::Position::FollowCursor,
        )
        .style(container::rounded_box)
        .into()
    } else {
        action.into()
    }
}

pub fn search_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E800}')
}
pub fn picture_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E801}')
}
pub fn th_large_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E802}')
}
pub fn ok_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E803}')
}
pub fn cancel_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E804}')
}
pub fn help_circled_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E805}')
}
pub fn pin_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E806}')
}
pub fn download_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E807}')
}
pub fn upload_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E808}')
}
pub fn pencil_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E809}')
}
pub fn retweet_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E80A}')
}
pub fn trash_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0F1F8}')
}
pub fn file_pdf_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0F1C1}')
}
pub fn file_image_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0F1C5}')
}
pub fn folder_open_empty_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0F115}')
}
pub fn cog_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E80B}')
}
pub fn resize_full_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E80C}')
}
pub fn resize_small_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E80D}')
}
pub fn down_open_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E80E}')
}
pub fn up_open_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E80F}')
}
pub fn refresh_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E810}')
}
pub fn vsplit_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E811}')
}
pub fn hsplit_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E812}')
}
pub fn unpin_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E813}')
}
pub fn nadi_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E814}')
}
pub fn angle_double_up_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0F102}')
}
pub fn angle_double_down_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0F103}')
}
pub fn arrows_cw_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0E810}')
}
pub fn github_circled_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0F09B}')
}
pub fn terminal_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0F120}')
}
