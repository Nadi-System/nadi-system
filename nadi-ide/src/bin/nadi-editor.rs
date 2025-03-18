use nadi::editor::Editor;

fn main() -> iced::Result {
    iced::application("NADI Editor", Editor::update, Editor::view)
        .font(include_bytes!("../../fonts/icons.ttf").as_slice())
        .theme(Editor::theme)
        .run()
}
