use nadi::icons;
use nadi::svg::SvgView;

fn main() -> iced::Result {
    iced::application("NADI Svg View", SvgView::update, SvgView::view)
        .font(icons::FONT)
        .theme(SvgView::theme)
        .run()
}
