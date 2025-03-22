use iced::widget::{button, center, container, text, tooltip};
use iced::{Element, Font};

pub static FONT: &[u8] = include_bytes!("../fonts/icons.ttf");

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

macro_rules! font_icon {
    ($name:ident, $ch:literal) => {
        pub fn $name<'a, Message>() -> Element<'a, Message> {
            icon($ch)
        }
    };
}

font_icon!(search_icon, '\u{0E800}');
font_icon!(picture_icon, '\u{0E801}');
font_icon!(th_large_icon, '\u{0E802}');
font_icon!(ok_icon, '\u{0E803}');
font_icon!(cancel_icon, '\u{0E804}');
font_icon!(help_icon, '\u{0E805}');
font_icon!(pin_icon, '\u{0E806}');
font_icon!(download_icon, '\u{0E807}');
font_icon!(upload_icon, '\u{0E808}');
font_icon!(pencil_icon, '\u{0E809}');
font_icon!(retweet_icon, '\u{0E80A}');
font_icon!(cog_icon, '\u{0E80B}');
font_icon!(resize_full_icon, '\u{0E80C}');
font_icon!(resize_small_icon, '\u{0E80D}');
font_icon!(arrow_down_icon, '\u{0E80E}');
font_icon!(arrow_up_icon, '\u{0E80F}');
font_icon!(refresh_icon, '\u{0E810}');
font_icon!(vsplit_icon, '\u{0E811}');
font_icon!(hsplit_icon, '\u{0E812}');
font_icon!(unpin_icon, '\u{0E813}');
font_icon!(nadi_icon, '\u{0E814}');
font_icon!(top_icon, '\u{0E815}');
font_icon!(bottom_icon, '\u{0E816}');
font_icon!(up_icon, '\u{0E817}');
font_icon!(down_icon, '\u{0E818}');
font_icon!(comment_icon, '\u{0E819}');
// font_icon!(funchelp_icon, '\u{0E81A}');
font_icon!(image_icon, '\u{0E81b}');
font_icon!(run_all_icon, '\u{0E81c}');
font_icon!(run_step_icon, '\u{0E81d}');
font_icon!(run_line_icon, '\u{0E81e}');
font_icon!(double_up_icon, '\u{0F102}');
font_icon!(double_down_icon, '\u{0F103}');
font_icon!(github_icon, '\u{0F09B}');
font_icon!(terminal_icon, '\u{0F120}');
font_icon!(trash_icon, '\u{0F1F8}');
font_icon!(file_pdf_icon, '\u{0F1C1}');
font_icon!(file_image_icon, '\u{0F1C5}');
font_icon!(open_icon, '\u{0F115}');
