use iced::theme::{Base, Mode};
use iced::{Color, Shadow, Theme, Vector, color};

#[allow(dead_code)]
pub(super) struct Tokens {
    pub brand: Color,
    pub accent: Color,
    pub bg_default: Color,
    pub bg_subtle: Color,
    pub bg_muted: Color,
    pub bg_frame: Color,
    pub bg_raised: Color,
    pub bg_inverse: Color,
    pub bg_overlay: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_tertiary: Color,
    pub text_disabled: Color,
    pub text_accent: Color,
    pub text_on_accent: Color,
    pub text_inverse: Color,
    pub border_default: Color,
    pub border_accent: Color,
    pub border_focus: Color,
    pub fill_accent: Color,
    pub fill_accent_subtle: Color,
    pub fill_neutral: Color,
    pub fill_neutral_strong: Color,
    pub fill_hover: Color,
    pub fill_pressed: Color,
    pub fill_accent_hover: Color,
    pub fill_accent_pressed: Color,
    pub status_success: Color,
    pub status_warning: Color,
    pub status_danger: Color,
    pub shadow_sm: Shadow,
    pub shadow_lg: Shadow,
    pub disabled_opacity: f32,
}

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

const LIGHT: Tokens = Tokens {
    brand: color!(0xFFCB50),
    accent: color!(0xBE7E12),
    bg_default: color!(0xFFFCF7),
    bg_subtle: color!(0xFBF7F0),
    bg_muted: color!(0xF3EDE3),
    bg_frame: color!(0xF5F0E8),
    bg_raised: color!(0xFFFFFF),
    bg_inverse: color!(0x1F1A14),
    bg_overlay: color!(0x1F1A14, 0.18),
    text_primary: color!(0x1F1A14),
    text_secondary: color!(0x57534C),
    text_tertiary: color!(0x706B63),
    text_disabled: color!(0xBEB8AE),
    text_accent: color!(0xA35300),
    text_on_accent: color!(0x1F1A14),
    text_inverse: color!(0xFFFCF7),
    border_default: color!(0xE6E0D6),
    border_accent: color!(0xF0BF4B),
    border_focus: color!(0xBE7E12),
    fill_accent: color!(0xFFDB85),
    fill_accent_subtle: color!(0xFDF1D3),
    fill_neutral: color!(0xF1ECE3),
    fill_neutral_strong: color!(0x969188),
    fill_hover: color!(0xF3EDE3),
    fill_pressed: color!(0xE6E0D6),
    fill_accent_hover: color!(0xFFCB50),
    fill_accent_pressed: color!(0xF0BF4B),
    status_success: color!(0x2F7A3F),
    status_warning: color!(0x9A5F08),
    status_danger: color!(0xB03A22),
    shadow_sm: Shadow {
        color: color!(0x281E14, 0.12),
        offset: Vector::new(0.0, 1.0),
        blur_radius: 2.0,
    },
    shadow_lg: Shadow {
        color: color!(0x281E14, 0.16),
        offset: Vector::new(0.0, 24.0),
        blur_radius: 60.0,
    },
    disabled_opacity: 0.38,
};

const DARK: Tokens = Tokens {
    brand: color!(0xFFCB50),
    accent: color!(0xF0B33A),
    bg_default: color!(0x181715),
    bg_subtle: color!(0x1C1B19),
    bg_muted: color!(0x272522),
    bg_frame: color!(0x121110),
    bg_raised: color!(0x211F1C),
    bg_inverse: color!(0xF2EFEA),
    bg_overlay: color!(0x121110, 0.6),
    text_primary: color!(0xF2EFEA),
    text_secondary: color!(0xBDB8B0),
    text_tertiary: color!(0xA09B93),
    text_disabled: color!(0x4F4B45),
    text_accent: color!(0xFFCB50),
    text_on_accent: color!(0xFFCB50),
    text_inverse: color!(0x181715),
    border_default: color!(0x34312C),
    border_accent: color!(0x77612C),
    border_focus: color!(0xF0B33A),
    fill_accent: color!(0x45391C),
    fill_accent_subtle: color!(0x3A311C),
    fill_neutral: color!(0x2A2825),
    fill_neutral_strong: color!(0x6E6A62),
    fill_hover: color!(0x272522),
    fill_pressed: color!(0x34312C),
    fill_accent_hover: color!(0x52431F),
    fill_accent_pressed: color!(0x5E4D23),
    status_success: color!(0x8FCB7A),
    status_warning: color!(0xF0B86E),
    status_danger: color!(0xE66C5C),
    shadow_sm: Shadow {
        color: color!(0x000000, 0.4),
        offset: Vector::new(0.0, 1.0),
        blur_radius: 2.0,
    },
    shadow_lg: Shadow {
        color: color!(0x000000, 0.55),
        offset: Vector::new(0.0, 30.0),
        blur_radius: 80.0,
    },
    disabled_opacity: 0.28,
};

#[allow(dead_code)]
pub(super) mod space {
    pub(in crate::gui) const S0: f32 = 0.0;
    pub(in crate::gui) const S025: f32 = 2.0;
    pub(in crate::gui) const S050: f32 = 4.0;
    pub(in crate::gui) const S075: f32 = 6.0;
    pub(in crate::gui) const S100: f32 = 8.0;
    pub(in crate::gui) const S150: f32 = 12.0;
    pub(in crate::gui) const S200: f32 = 16.0;
    pub(in crate::gui) const S250: f32 = 20.0;
    pub(in crate::gui) const S300: f32 = 24.0;
    pub(in crate::gui) const S400: f32 = 32.0;
}

#[allow(dead_code)]
pub(super) mod radius {
    pub(in crate::gui) const XS: f32 = 2.0;
    pub(in crate::gui) const SM: f32 = 4.0;
    pub(in crate::gui) const MD: f32 = 6.0;
    pub(in crate::gui) const LG: f32 = 8.0;
    pub(in crate::gui) const XL: f32 = 10.0;
    pub(in crate::gui) const FULL: f32 = 9999.0;
}

#[allow(dead_code)]
pub(super) mod weight {
    use iced::font::Weight;

    pub(in crate::gui) const REGULAR: Weight = Weight::Normal;
    pub(in crate::gui) const MEDIUM: Weight = Weight::Medium;
    pub(in crate::gui) const SEMIBOLD: Weight = Weight::Semibold;
    pub(in crate::gui) const BOLD: Weight = Weight::Bold;
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
pub(super) mod font_size {
    pub(in crate::gui) const XS2: f32 = 10.0;
    pub(in crate::gui) const XS: f32 = 11.0;
    pub(in crate::gui) const SM: f32 = 12.0;
    pub(in crate::gui) const MD: f32 = 13.0;
    pub(in crate::gui) const LG: f32 = 14.0;
    pub(in crate::gui) const XL: f32 = 16.0;
    pub(in crate::gui) const XL2: f32 = 18.0;
    pub(in crate::gui) const XL3: f32 = 24.0;
    pub(in crate::gui) const XL4: f32 = 30.0;
    pub(in crate::gui) const XL5: f32 = 40.0;
    pub(in crate::gui) const TIMER: f32 = 68.0;
}

#[allow(dead_code)]
pub(super) mod line_height {
    use iced::widget::text::LineHeight;

    pub(in crate::gui) const NONE: LineHeight = LineHeight::Relative(1.0);
    pub(in crate::gui) const TIGHT: LineHeight = LineHeight::Relative(1.25);
    pub(in crate::gui) const SNUG: LineHeight = LineHeight::Relative(1.4);
    pub(in crate::gui) const DEFAULT: LineHeight = LineHeight::Relative(1.5);
    pub(in crate::gui) const RELAXED: LineHeight = LineHeight::Relative(1.6);
}

#[allow(dead_code)]
pub(super) mod tracking {
    pub(in crate::gui) const NONE: f32 = 0.0;
    pub(in crate::gui) const CAPS: f32 = 0.12;
    pub(in crate::gui) const HEADING: f32 = -0.02;
    pub(in crate::gui) const DISPLAY: f32 = -0.03;
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
