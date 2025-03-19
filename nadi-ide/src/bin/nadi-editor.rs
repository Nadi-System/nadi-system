use nadi::editor::Editor;
use nadi::icons;

fn main() -> iced::Result {
    iced::application("NADI Editor", Editor::update, Editor::view)
        .font(icons::FONT)
        .theme(Editor::theme)
        .run()
}
