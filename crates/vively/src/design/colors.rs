use gpui_kit::{
    App, BoxShadow, Hsla, WindowAppearance,
    component::{Theme, ThemeMode},
    hsla, point, px, };
use vively_core::settings::Appearance;

pub use vively_app::palette::{DEFAULT_TINT, RgbColor, mix_oklab, parse_hex_color};
use vively_app::palette::PaletteTokens;

/// gpui conversion for the shared colour type.
pub trait ToHsla {
    fn to_hsla(self) -> Hsla;
}

impl ToHsla for RgbColor {
    fn to_hsla(self) -> Hsla {
        let r = self.r.clamp(0.0, 1.0);
        let g = self.g.clamp(0.0, 1.0);
        let b = self.b.clamp(0.0, 1.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) * 0.5;
        let d = max - min;
        if d <= 1e-6 {
            return hsla(0.0, 0.0, l, self.a.clamp(0.0, 1.0));
        }
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let mut h = if (max - r).abs() <= 1e-6 {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if (max - g).abs() <= 1e-6 {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h /= 6.0;
        hsla(h.clamp(0.0, 1.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0), self.a.clamp(0.0, 1.0))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub is_dark: bool,
    pub tint: RgbColor,
    pub is_neutral: bool,
    pub base: Hsla,
    pub ground: Hsla,
    pub ground_rgb: RgbColor,
    pub rail: Hsla,
    pub raised: Hsla,
    pub stage_bg: Hsla,
    pub dock: Hsla,
    pub fill: Hsla,
    pub fill_2: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
    pub line: Hsla,
    pub art_ring: Hsla,
    pub ink: Hsla,
    pub ink_2: Hsla,
    pub ink_3: Hsla,
    pub hi: Hsla,
    pub hi_rgb: RgbColor,
    pub cta_bg: Hsla,
    pub cta_fg: Hsla,
    pub cta_hover: Hsla,
    pub live_dot: Hsla,
    pub danger: Hsla,
    pub scrim: Hsla,
}

impl Palette {
    pub fn resolve(is_dark: bool, tint: RgbColor, is_neutral: bool) -> Self {
        let t = PaletteTokens::resolve(is_dark, tint, is_neutral);
        Self {
            is_dark,
            tint,
            is_neutral,
            base: t.base.to_hsla(),
            ground: t.ground.to_hsla(),
            ground_rgb: t.ground,
            rail: t.rail.to_hsla(),
            raised: t.raised.to_hsla(),
            stage_bg: t.stage_bg.to_hsla(),
            dock: t.dock.to_hsla(),
            fill: t.fill.to_hsla(),
            fill_2: t.fill_2.to_hsla(),
            hover: t.hover.to_hsla(),
            active: t.active.to_hsla(),
            line: t.line.to_hsla(),
            art_ring: t.art_ring.to_hsla(),
            ink: t.ink.to_hsla(),
            ink_2: t.ink_2.to_hsla(),
            ink_3: t.ink_3.to_hsla(),
            hi: t.hi.to_hsla(),
            hi_rgb: t.hi,
            cta_bg: t.cta_bg.to_hsla(),
            cta_fg: t.cta_fg.to_hsla(),
            cta_hover: t.cta_hover.to_hsla(),
            live_dot: t.live_dot.to_hsla(),
            danger: t.danger.to_hsla(),
            scrim: t.scrim.to_hsla(),
        }
    }

    pub fn hero_banner_bg(&self, col_tint: Option<RgbColor>) -> Hsla {
        let tint = col_tint.unwrap_or(self.tint);
        let pct = if self.is_dark { 22.0 } else { 16.0 };
        mix_oklab(tint, pct, self.ground_rgb).to_hsla()
    }

    pub fn wave_hero_bg(&self) -> Hsla {
        mix_oklab(self.hi_rgb, 13.0, self.ground_rgb).to_hsla()
    }

    pub fn shadow_pop(&self) -> Vec<BoxShadow> {
        let (a1, a2, b2) = if self.is_dark {
            (0.30, 0.42, 40.0)
        } else {
            (0.08, 0.14, 36.0)
        };
        vec![
            BoxShadow {
                color: hsla(0.0, 0.0, 0.0, a1),
                offset: point(px(0.0), px(1.0)),
                blur_radius: px(2.0),
                spread_radius: px(0.0),
                inset: false,
            },
            BoxShadow {
                color: hsla(0.0, 0.0, 0.0, a2),
                offset: point(px(0.0), px(16.0)),
                blur_radius: px(b2),
                spread_radius: px(0.0),
                inset: false,
            },
        ]
    }

    pub fn shadow_art(&self) -> Vec<BoxShadow> {
        let (a1, a2, y2, b2) = if self.is_dark {
            (0.30, 0.34, 10.0, 28.0)
        } else {
            (0.10, 0.12, 8.0, 22.0)
        };
        vec![
            BoxShadow {
                color: hsla(0.0, 0.0, 0.0, a1),
                offset: point(px(0.0), px(1.0)),
                blur_radius: px(2.0),
                spread_radius: px(0.0),
                inset: false,
            },
            BoxShadow {
                color: hsla(0.0, 0.0, 0.0, a2),
                offset: point(px(0.0), px(y2)),
                blur_radius: px(b2),
                spread_radius: px(0.0),
                inset: false,
            },
        ]
    }

    pub fn shadow_stage_cover(&self) -> Vec<BoxShadow> {
        vec![
            BoxShadow {
                color: hsla(0.0, 0.0, 0.0, 0.20),
                offset: point(px(0.0), px(2.0)),
                blur_radius: px(4.0),
                spread_radius: px(0.0),
                inset: false,
            },
            BoxShadow {
                color: hsla(0.0, 0.0, 0.0, 0.35),
                offset: point(px(0.0), px(30.0)),
                blur_radius: px(60.0),
                spread_radius: px(0.0),
                inset: false,
            },
        ]
    }
}

pub fn resolve_is_dark(
    appearance: Appearance,
    window_appearance: Option<WindowAppearance>,
) -> bool {
    match appearance {
        Appearance::Dark => true,
        Appearance::Light => false,
        Appearance::System => !matches!(
            window_appearance,
            Some(WindowAppearance::Light | WindowAppearance::VibrantLight)
        ),
    }
}

pub fn sync_gpui_theme(palette: &Palette, cx: &mut App) {
    let mode = if palette.is_dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    if cx.global::<Theme>().mode != mode {
        Theme::change(mode, None, cx);
    }
    let theme = Theme::global_mut(cx);
    theme.colors.background = palette.ground;
    theme.colors.foreground = palette.ink;
    theme.colors.muted_foreground = palette.ink_2;
    theme.colors.border = palette.line;
    theme.colors.input = palette.fill;
    theme.colors.caret = palette.ink;
    theme.colors.selection = palette.hi.opacity(0.28);
    theme.colors.primary = palette.hi;
    theme.colors.primary_foreground = palette.cta_fg;
    theme.colors.popover = palette.raised;
    theme.colors.popover_foreground = palette.ink;
}
