mod app;
mod theme;
mod views;

use app::Gui;
use iced::font::{Family, Weight};
use iced::{Font, Pixels, Settings};

const INTER_BOLD: Font = Font {
    family: Family::Name("Inter"),
    weight: Weight::Bold,
    ..Font::DEFAULT
};

const JETBRAINS_MONO: Font = Font::with_name("JetBrains Mono");

pub(crate) fn run() -> anyhow::Result<()> {
    iced::daemon(Gui::new, Gui::update, Gui::view)
        .subscription(Gui::subscription)
        .theme(Gui::theme)
        .title("Nirvana")
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
        .run()?;
    Ok(())
}
