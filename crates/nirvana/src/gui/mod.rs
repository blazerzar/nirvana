mod header;

use header::header;
use iced::font::{Family, Weight};
use iced::widget::{Column, container};
use iced::{Color, Element, Font, Length, Size, color};

const BG_DEFAULT: Color = color!(0xFFFCF7);
const TEXT_PRIMARY: Color = color!(0x1F1A14);
const BORDER_DEFAULT: Color = color!(0xE6E0D6);

const INTER_BOLD: Font = Font {
    family: Family::Name("Inter"),
    weight: Weight::Bold,
    ..Font::DEFAULT
};

pub(crate) fn run() -> iced::Result {
    iced::application(Gui::default, Gui::update, Gui::view)
        .title("Nirvana")
        .window_size(Size::new(960.0, 540.0))
        .font(include_bytes!("../../assets/fonts/Inter/Bold.ttf").as_slice())
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
            .style(|_| container::background(BG_DEFAULT))
            .into()
    }
}
