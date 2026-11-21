mod app;
mod design_system;
mod views;

use app::Gui;
use iced::{Pixels, Settings};

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
            default_font: design_system::font::MONO,
            default_text_size: Pixels(13.0),
            ..Settings::default()
        })
        .run()?;
    Ok(())
}
