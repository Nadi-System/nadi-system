use nadi::editor::Editor;

fn main() -> iced::Result {
    iced::application("NADI Editor", Editor::update, Editor::view)
        .theme(Editor::theme)
        .run()
}
