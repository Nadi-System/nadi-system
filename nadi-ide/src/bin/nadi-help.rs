use nadi_ide::help::MdHelp;
use nadi_ide::icons;

fn main() -> iced::Result {
    iced::application("NADI Help", MdHelp::update, MdHelp::view)
        .font(icons::FONT)
        .theme(MdHelp::theme)
        .run()
}
