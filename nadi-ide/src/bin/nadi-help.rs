use nadi::help::MdHelp;
use nadi::icons;

fn main() -> iced::Result {
    iced::application("NADI Help", MdHelp::update, MdHelp::view)
        .font(icons::FONT)
        .theme(MdHelp::theme)
        .run()
}
