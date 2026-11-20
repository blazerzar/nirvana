mod header;
mod theme;

use header::header;
use iced::font::{Family, Weight};
use iced::theme::Mode;
use iced::widget::{Column, container};
use iced::{Element, Font, Length, Pixels, Settings, Size, Subscription, Task, keyboard, system};
use nirvana_core::{Nirvana, Theme};
use theme::{DARK, LIGHT, Tokens};

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

struct Gui {
    theme: Theme,
    system_mode: Mode,
}

#[derive(Debug, Clone)]
enum Message {
    SystemThemeChanged(Mode),
    ToggleTheme,
}

impl Gui {
    fn new(theme: Theme) -> (Self, Task<Message>) {
        let gui = Self {
            theme,
            system_mode: Mode::None,
        };
        (gui, system::theme().map(Message::SystemThemeChanged))
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::SystemThemeChanged(mode) => self.system_mode = mode,
            Message::ToggleTheme => {
                self.theme = if self.is_dark() {
                    Theme::Light
                } else {
                    Theme::Dark
                };
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            system::theme_changes().map(Message::SystemThemeChanged),
            keyboard::listen().filter_map(toggle_theme_shortcut),
        ])
    }

    fn is_dark(&self) -> bool {
        match self.theme {
            Theme::System => self.system_mode == Mode::Dark,
            Theme::Light => false,
            Theme::Dark => true,
        }
    }

    fn tokens(&self) -> &'static Tokens {
        if self.is_dark() { &DARK } else { &LIGHT }
    }

    fn view(&self) -> Element<'_, Message> {
        let tokens = self.tokens();
        container(Column::new().push(header(tokens)))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::background(tokens.bg_default))
            .into()
    }
}

fn toggle_theme_shortcut(event: keyboard::Event) -> Option<Message> {
    let keyboard::Event::KeyPressed {
        key,
        physical_key,
        modifiers,
        repeat: false,
        ..
    } = event
    else {
        return None;
    };
    let is_t = key
        .to_latin(physical_key)
        .is_some_and(|c| c.eq_ignore_ascii_case(&'t'));
    (modifiers.command() && modifiers.shift() && is_t).then_some(Message::ToggleTheme)
}
