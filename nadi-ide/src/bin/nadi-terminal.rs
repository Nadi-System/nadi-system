use nadi::icons;
use nadi::terminal::Terminal;

fn main() -> iced::Result {
    iced::application("NADI Terminal", Terminal::update, Terminal::view)
        .font(icons::FONT)
        .theme(Terminal::theme)
        .run()
}
