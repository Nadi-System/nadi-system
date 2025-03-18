use nadi::svg::SvgView;

fn main() -> iced::Result {
    iced::application("NADI Svg View", SvgView::update, SvgView::view)
        .font(include_bytes!("../../fonts/icons.ttf").as_slice())
        .theme(SvgView::theme)
        .run()
}
