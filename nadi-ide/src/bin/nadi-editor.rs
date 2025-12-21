use nadi_ide::editor::Editor;
use nadi_ide::icons;

fn main() -> iced::Result {
    iced::application(Editor::default, Editor::update, Editor::view)
        .font(icons::FONT)
        .theme(Editor::theme)
        .subscription(Editor::subscription)
        .run()
}
