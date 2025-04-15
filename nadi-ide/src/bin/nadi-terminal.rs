use nadi_ide::icons;
use nadi_ide::terminal::Terminal;

fn main() -> iced::Result {
    iced::application("NADI Terminal", Terminal::update, Terminal::view)
        .font(icons::FONT)
        .theme(Terminal::theme)
        .run()
}
