use iced::Theme;
use iced::theme::{Base, Mode};

use crate::tokens::{DARK, LIGHT};
pub(super) use crate::tokens::{Tokens, font_size, radius, space, tracking};

impl Tokens {
    pub(super) fn opacity(&self, enabled: bool) -> f32 {
        if enabled { 1.0 } else { self.disabled_opacity }
    }
}

pub(super) fn tokens(theme: &Theme) -> &'static Tokens {
    if theme.mode() == Mode::Dark {
        &DARK
    } else {
        &LIGHT
    }
}

#[allow(dead_code)]
pub(super) mod weight {
    use iced::font::Weight;

    use crate::tokens::weight;

    const fn iced(value: u16) -> Weight {
        match value {
            100 => Weight::Thin,
            200 => Weight::ExtraLight,
            300 => Weight::Light,
            400 => Weight::Normal,
            500 => Weight::Medium,
            600 => Weight::Semibold,
            700 => Weight::Bold,
            800 => Weight::ExtraBold,
            900 => Weight::Black,
            _ => panic!("font weight must be a multiple of 100 from 100 to 900"),
        }
    }

    pub(in crate::gui) const REGULAR: Weight = iced(weight::REGULAR);
    pub(in crate::gui) const MEDIUM: Weight = iced(weight::MEDIUM);
    pub(in crate::gui) const SEMIBOLD: Weight = iced(weight::SEMIBOLD);
    pub(in crate::gui) const BOLD: Weight = iced(weight::BOLD);
}

#[allow(dead_code)]
pub(super) mod font {
    use iced::Font;
    use iced::font::{Family, Weight};

    use super::weight::{BOLD, MEDIUM, REGULAR, SEMIBOLD};

    const fn mono(weight: Weight) -> Font {
        Font {
            weight,
            ..Font::with_name("JetBrains Mono")
        }
    }

    pub(in crate::gui) const INTER_BOLD: Font = Font {
        family: Family::Name("Inter"),
        weight: BOLD,
        ..Font::DEFAULT
    };
    pub(in crate::gui) const MONO: Font = mono(REGULAR);
    pub(in crate::gui) const MONO_MEDIUM: Font = mono(MEDIUM);
    pub(in crate::gui) const MONO_SEMIBOLD: Font = mono(SEMIBOLD);
    pub(in crate::gui) const MONO_BOLD: Font = mono(BOLD);
}

#[allow(dead_code)]
pub(super) mod line_height {
    use iced::widget::text::LineHeight;

    use crate::tokens::line_height;

    pub(in crate::gui) const NONE: LineHeight = LineHeight::Relative(line_height::NONE);
    pub(in crate::gui) const TIGHT: LineHeight = LineHeight::Relative(line_height::TIGHT);
    pub(in crate::gui) const SNUG: LineHeight = LineHeight::Relative(line_height::SNUG);
    pub(in crate::gui) const DEFAULT: LineHeight = LineHeight::Relative(line_height::DEFAULT);
    pub(in crate::gui) const RELAXED: LineHeight = LineHeight::Relative(line_height::RELAXED);
}

#[allow(dead_code)]
pub(super) mod typography {
    use iced::Font;
    use iced::widget::text::{IntoFragment, LineHeight, Text};

    use super::font::{INTER_BOLD, MONO, MONO_BOLD, MONO_MEDIUM, MONO_SEMIBOLD};
    use super::font_size::{LG, MD, SM, TIMER, XL, XL2, XL3, XL4, XL5, XS, XS2};
    use super::line_height::{DEFAULT, NONE, RELAXED, SNUG, TIGHT};
    use super::tracking::{self, CAPS, DISPLAY, HEADING};

    #[derive(Clone, Copy)]
    pub(in crate::gui) struct TextStyle {
        pub font: Font,
        pub size: f32,
        pub line_height: LineHeight,
        pub tracking: f32,
    }

    impl TextStyle {
        pub(in crate::gui) fn text<'a>(self, content: impl IntoFragment<'a>) -> Text<'a> {
            iced::widget::text(content)
                .font(self.font)
                .size(self.size)
                .line_height(self.line_height)
        }
    }

    const fn style(font: Font, size: f32, line_height: LineHeight, tracking: f32) -> TextStyle {
        TextStyle {
            font,
            size,
            line_height,
            tracking,
        }
    }

    pub(in crate::gui) const DISPLAY_TIMER: TextStyle = style(MONO_MEDIUM, TIMER, NONE, DISPLAY);
    pub(in crate::gui) const DISPLAY_KEY: TextStyle = style(MONO_BOLD, XL5, NONE, DISPLAY);
    pub(in crate::gui) const STAT: TextStyle = style(MONO_SEMIBOLD, XL4, NONE, DISPLAY);
    pub(in crate::gui) const HEADING_PAGE: TextStyle = style(INTER_BOLD, XL3, TIGHT, HEADING);
    pub(in crate::gui) const HEADING_SCREEN: TextStyle = style(INTER_BOLD, XL2, TIGHT, HEADING);
    pub(in crate::gui) const HEADING_SM: TextStyle = style(INTER_BOLD, XL, TIGHT, HEADING);
    pub(in crate::gui) const TITLE: TextStyle = style(MONO, XL, SNUG, tracking::NONE);
    pub(in crate::gui) const TITLE_SM: TextStyle = style(MONO_SEMIBOLD, LG, SNUG, tracking::NONE);
    pub(in crate::gui) const LABEL: TextStyle = style(MONO_MEDIUM, MD, DEFAULT, tracking::NONE);
    pub(in crate::gui) const LABEL_STRONG: TextStyle =
        style(MONO_SEMIBOLD, MD, DEFAULT, tracking::NONE);
    pub(in crate::gui) const BODY_LG: TextStyle = style(MONO, MD, DEFAULT, tracking::NONE);
    pub(in crate::gui) const PARAGRAPH: TextStyle = style(MONO, MD, RELAXED, tracking::NONE);
    pub(in crate::gui) const LABEL_SM: TextStyle = style(MONO_MEDIUM, SM, DEFAULT, tracking::NONE);
    pub(in crate::gui) const BODY: TextStyle = style(MONO, SM, DEFAULT, tracking::NONE);
    pub(in crate::gui) const BODY_STRONG: TextStyle =
        style(MONO_SEMIBOLD, SM, DEFAULT, tracking::NONE);
    pub(in crate::gui) const CAPTION: TextStyle = style(MONO, XS, SNUG, tracking::NONE);
    pub(in crate::gui) const CAPTION_STRONG: TextStyle =
        style(MONO_SEMIBOLD, XS, SNUG, tracking::NONE);
    pub(in crate::gui) const KBD: TextStyle = style(MONO_MEDIUM, XS, DEFAULT, tracking::NONE);
    pub(in crate::gui) const MICRO: TextStyle = style(MONO, XS2, SNUG, tracking::NONE);
    pub(in crate::gui) const LABEL_CAPS: TextStyle = style(MONO, XS2, SNUG, CAPS);
}
