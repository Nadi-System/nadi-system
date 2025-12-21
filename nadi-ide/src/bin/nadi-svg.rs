use nadi_ide::icons;
use nadi_ide::svg::SvgView;

fn main() -> iced::Result {
    iced::application(SvgView::default, SvgView::update, SvgView::view)
        .font(icons::FONT)
        .theme(SvgView::theme)
        .run()
}
