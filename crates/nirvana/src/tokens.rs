use iced::{Color, Shadow, Vector, color};

pub(crate) struct Tokens {
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
    fn colors(&self) -> [(&'static str, Color); 30] {
        [
            ("brand", self.brand),
            ("accent", self.accent),
            ("bg-default", self.bg_default),
            ("bg-subtle", self.bg_subtle),
            ("bg-muted", self.bg_muted),
            ("bg-frame", self.bg_frame),
            ("bg-raised", self.bg_raised),
            ("bg-inverse", self.bg_inverse),
            ("bg-overlay", self.bg_overlay),
            ("text-primary", self.text_primary),
            ("text-secondary", self.text_secondary),
            ("text-tertiary", self.text_tertiary),
            ("text-disabled", self.text_disabled),
            ("text-accent", self.text_accent),
            ("text-on-accent", self.text_on_accent),
            ("text-inverse", self.text_inverse),
            ("border-default", self.border_default),
            ("border-accent", self.border_accent),
            ("border-focus", self.border_focus),
            ("fill-accent", self.fill_accent),
            ("fill-accent-subtle", self.fill_accent_subtle),
            ("fill-neutral", self.fill_neutral),
            ("fill-neutral-strong", self.fill_neutral_strong),
            ("fill-hover", self.fill_hover),
            ("fill-pressed", self.fill_pressed),
            ("fill-accent-hover", self.fill_accent_hover),
            ("fill-accent-pressed", self.fill_accent_pressed),
            ("status-success", self.status_success),
            ("status-warning", self.status_warning),
            ("status-danger", self.status_danger),
        ]
    }

    pub(crate) fn css(&self) -> String {
        let colors = self
            .colors()
            .map(|(name, color)| format!("--{name}: {};\n", rgb(color)));
        let shadows =
            [("shadow-sm", self.shadow_sm), ("shadow-lg", self.shadow_lg)].map(|(name, shadow)| {
                format!(
                    "--{name}: {}px {}px {}px {};\n",
                    shadow.offset.x,
                    shadow.offset.y,
                    shadow.blur_radius,
                    rgb(shadow.color)
                )
            });
        let opacity = format!("--disabled-opacity: {};\n", self.disabled_opacity);
        colors.into_iter().chain(shadows).chain([opacity]).collect()
    }
}

fn rgb(color: Color) -> String {
    let [r, g, b, _] = color.into_rgba8();
    if color.a < 1.0 {
        format!("rgb({r} {g} {b} / {})", color.a)
    } else {
        format!("rgb({r} {g} {b})")
    }
}

fn scales_css() -> String {
    let px = [
        ("space-0", space::S0),
        ("space-025", space::S025),
        ("space-050", space::S050),
        ("space-075", space::S075),
        ("space-100", space::S100),
        ("space-150", space::S150),
        ("space-200", space::S200),
        ("space-250", space::S250),
        ("space-300", space::S300),
        ("space-400", space::S400),
        ("radius-xs", radius::XS),
        ("radius-sm", radius::SM),
        ("radius-md", radius::MD),
        ("radius-lg", radius::LG),
        ("radius-xl", radius::XL),
        ("radius-full", radius::FULL),
        ("font-size-xs2", font_size::XS2),
        ("font-size-xs", font_size::XS),
        ("font-size-sm", font_size::SM),
        ("font-size-md", font_size::MD),
        ("font-size-lg", font_size::LG),
        ("font-size-xl", font_size::XL),
        ("font-size-xl2", font_size::XL2),
        ("font-size-xl3", font_size::XL3),
        ("font-size-xl4", font_size::XL4),
        ("font-size-xl5", font_size::XL5),
        ("font-size-timer", font_size::TIMER),
    ]
    .map(|(name, value)| format!("--{name}: {value}px;\n"));
    let em = [
        ("tracking-none", tracking::NONE),
        ("tracking-caps", tracking::CAPS),
        ("tracking-heading", tracking::HEADING),
        ("tracking-display", tracking::DISPLAY),
    ]
    .map(|(name, value)| format!("--{name}: {value}em;\n"));
    let line_heights = [
        ("line-height-none", line_height::NONE),
        ("line-height-tight", line_height::TIGHT),
        ("line-height-snug", line_height::SNUG),
        ("line-height-default", line_height::DEFAULT),
        ("line-height-relaxed", line_height::RELAXED),
    ]
    .map(|(name, value)| format!("--{name}: {value};\n"));
    let weights = [
        ("weight-regular", weight::REGULAR),
        ("weight-medium", weight::MEDIUM),
        ("weight-semibold", weight::SEMIBOLD),
        ("weight-bold", weight::BOLD),
    ]
    .map(|(name, value)| format!("--{name}: {value};\n"));
    px.into_iter()
        .chain(em)
        .chain(line_heights)
        .chain(weights)
        .collect()
}

#[allow(dead_code)]
pub(crate) fn tokens_css() -> String {
    format!(
        ":root {{\n{}{}}}\n\n@media (prefers-color-scheme: dark) {{\n:root {{\n{}}}\n}}\n",
        scales_css(),
        LIGHT.css(),
        DARK.css()
    )
}

pub(crate) const LIGHT: Tokens = Tokens {
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

pub(crate) const DARK: Tokens = Tokens {
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

pub(crate) mod space {
    pub(crate) const S0: f32 = 0.0;
    pub(crate) const S025: f32 = 2.0;
    pub(crate) const S050: f32 = 4.0;
    pub(crate) const S075: f32 = 6.0;
    pub(crate) const S100: f32 = 8.0;
    pub(crate) const S150: f32 = 12.0;
    pub(crate) const S200: f32 = 16.0;
    pub(crate) const S250: f32 = 20.0;
    pub(crate) const S300: f32 = 24.0;
    pub(crate) const S400: f32 = 32.0;
}

pub(crate) mod radius {
    pub(crate) const XS: f32 = 2.0;
    pub(crate) const SM: f32 = 4.0;
    pub(crate) const MD: f32 = 6.0;
    pub(crate) const LG: f32 = 8.0;
    pub(crate) const XL: f32 = 10.0;
    pub(crate) const FULL: f32 = 9999.0;
}

pub(crate) mod font_size {
    pub(crate) const XS2: f32 = 10.0;
    pub(crate) const XS: f32 = 11.0;
    pub(crate) const SM: f32 = 12.0;
    pub(crate) const MD: f32 = 13.0;
    pub(crate) const LG: f32 = 14.0;
    pub(crate) const XL: f32 = 16.0;
    pub(crate) const XL2: f32 = 18.0;
    pub(crate) const XL3: f32 = 24.0;
    pub(crate) const XL4: f32 = 30.0;
    pub(crate) const XL5: f32 = 40.0;
    pub(crate) const TIMER: f32 = 68.0;
}

pub(crate) mod tracking {
    pub(crate) const NONE: f32 = 0.0;
    pub(crate) const CAPS: f32 = 0.12;
    pub(crate) const HEADING: f32 = -0.02;
    pub(crate) const DISPLAY: f32 = -0.03;
}

pub(crate) mod weight {
    pub(crate) const REGULAR: u16 = 400;
    pub(crate) const MEDIUM: u16 = 500;
    pub(crate) const SEMIBOLD: u16 = 600;
    pub(crate) const BOLD: u16 = 700;
}

pub(crate) mod line_height {
    pub(crate) const NONE: f32 = 1.0;
    pub(crate) const TIGHT: f32 = 1.25;
    pub(crate) const SNUG: f32 = 1.4;
    pub(crate) const DEFAULT: f32 = 1.5;
    pub(crate) const RELAXED: f32 = 1.6;
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn css_has_theme_values() {
        assert!(LIGHT.css().contains("--bg-default: rgb(255 252 247);\n"));
        assert!(DARK.css().contains("--bg-overlay: rgb(18 17 16 / 0.6);\n"));
    }

    #[test]
    fn css_has_scale_values() {
        let css = scales_css();
        assert!(css.contains("--space-200: 16px;\n"));
        assert!(css.contains("--tracking-heading: -0.02em;\n"));
        assert!(css.contains("--weight-bold: 700;\n"));
    }

    #[test]
    fn css_names_are_unique() {
        let css = scales_css() + &LIGHT.css();
        let names: Vec<_> = css
            .lines()
            .filter_map(|line| line.split_once(':'))
            .collect();
        let unique: HashSet<_> = names.iter().map(|(name, _)| name).collect();
        assert_eq!(unique.len(), names.len());
    }
}
