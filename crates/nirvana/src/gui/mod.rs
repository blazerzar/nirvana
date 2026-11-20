mod header;
mod theme;

use header::header;
use iced::font::{Family, Weight};
use iced::widget::{Column, container};
use iced::{Element, Font, Length, Pixels, Settings, Size};
use theme::LIGHT;

const INTER_BOLD: Font = Font {
    family: Family::Name("Inter"),
    weight: Weight::Bold,
    ..Font::DEFAULT
};

const JETBRAINS_MONO: Font = Font::with_name("JetBrains Mono");

pub(crate) fn run() -> iced::Result {
    iced::application(Gui::default, Gui::update, Gui::view)
        .title("Nirvana")
        .window_size(Size::new(960.0, 540.0))
        .settings(Settings {
            fonts: vec![
                include_bytes!("../../assets/fonts/Inter/Bold.ttf").into(),
                include_bytes!("../../assets/fonts/JetBrainsMono/Regular.ttf").into(),
                include_bytes!("../../assets/fonts/JetBrainsMono/Medium.ttf").into(),
                include_bytes!("../../assets/fonts/JetBrainsMono/SemiBold.ttf").into(),
                include_bytes!("../../assets/fonts/JetBrainsMono/Bold.ttf").into(),
            ],
            default_font: JETBRAINS_MONO,
            default_text_size: Pixels(13.0),
            ..Settings::default()
        })
        .run()
}

#[derive(Default)]
struct Gui;

#[derive(Debug, Clone)]
enum Message {}

impl Gui {
    fn update(&mut self, message: Message) {
        match message {}
    }

    fn view(&self) -> Element<'_, Message> {
        container(Column::new().push(header()))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::background(LIGHT.bg_default))
            .into()
    }
}
