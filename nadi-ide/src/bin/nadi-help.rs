use nadi_ide::help::MdHelp;
use nadi_ide::icons;

fn main() -> iced::Result {
    iced::application(MdHelp::default, MdHelp::update, MdHelp::view)
        .font(icons::FONT)
        .theme(MdHelp::theme)
        .run()
}
