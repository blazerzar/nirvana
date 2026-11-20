mod app;
mod theme;
mod views;

use app::Gui;
use iced::font::{Family, Weight};
use iced::{Font, Pixels, Settings, Size};
use nirvana_core::{Nirvana, Theme};

const INTER_BOLD: Font = Font {
    family: Family::Name("Inter"),
    weight: Weight::Bold,
    ..Font::DEFAULT
};

const JETBRAINS_MONO: Font = Font::with_name("JetBrains Mono");

pub(crate) fn run() -> anyhow::Result<()> {
    let theme = load_theme()?;
    iced::application(move || Gui::new(theme), Gui::update, Gui::view)
        .subscription(Gui::subscription)
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
        .run()?;
    Ok(())
}

fn load_theme() -> anyhow::Result<Theme> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let theme = runtime.block_on(async {
        Nirvana::new()
            .await
            .map(|service| service.theme().unwrap_or_default())
    })?;
    Ok(theme)
}
