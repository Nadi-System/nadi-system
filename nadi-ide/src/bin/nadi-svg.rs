use nadi::svg::SvgView;

fn main() -> iced::Result {
    iced::application("NADI Svg View", SvgView::update, SvgView::view)
        .theme(SvgView::theme)
        .run()
}
