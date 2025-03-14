use nadi::help::MdHelp;

fn main() -> iced::Result {
    iced::application("NADI Help", MdHelp::update, MdHelp::view)
        .theme(MdHelp::theme)
        .run()
}
