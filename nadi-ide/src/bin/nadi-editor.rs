use nadi_ide::editor::Editor;
use nadi_ide::icons;

fn main() -> iced::Result {
    iced::application("NADI Editor", Editor::update, Editor::view)
        .font(icons::FONT)
        .theme(Editor::theme)
        .run()
}
