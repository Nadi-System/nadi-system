use nadi_ide::icons;
use nadi_ide::svg::SvgView;

fn main() -> iced::Result {
    iced::application("NADI Svg View", SvgView::update, SvgView::view)
        .font(icons::FONT)
        .theme(SvgView::theme)
        .run()
}
