use iced::widget::{Column, container};
use iced::{Color, Element, Length, Size, color};

const BG_DEFAULT: Color = color!(0xFFFCF7);

pub(crate) fn run() -> iced::Result {
    iced::application(Gui::default, Gui::update, Gui::view)
        .title("Nirvana")
        .window_size(Size::new(960.0, 540.0))
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
        container(Column::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::background(BG_DEFAULT))
            .into()
    }
}
