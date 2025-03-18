use nadi::help::MdHelp;

fn main() -> iced::Result {
    iced::application("NADI Help", MdHelp::update, MdHelp::view)
        .font(include_bytes!("../../fonts/icons.ttf").as_slice())
        .theme(MdHelp::theme)
        .run()
}
