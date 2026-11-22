//! Vively colour system shared by desktop (gpui) and Android (Compose).
//! Pure math: the artwork tint is OKLab-mixed into fixed base tones.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl RgbColor {
    pub const fn hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xff) as f32 / 255.0,
            g: ((hex >> 8) & 0xff) as f32 / 255.0,
            b: (hex & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }

    /// `0xRRGGBBAA`.
    pub const fn rgba_hex(v: u32) -> Self {
        Self {
            r: ((v >> 24) & 0xff) as f32 / 255.0,
            g: ((v >> 16) & 0xff) as f32 / 255.0,
            b: ((v >> 8) & 0xff) as f32 / 255.0,
            a: (v & 0xff) as f32 / 255.0,
        }
    }

    pub const fn rgba_u8(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a,
        }
    }

    pub fn with_alpha(mut self, a: f32) -> Self {
        self.a = a.clamp(0.0, 1.0);
        self
    }

    /// Packed `0xAARRGGBB` (Android `Color(Long)` / `Color(Int)` order).
    pub fn argb(self) -> u32 {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        (q(self.a) << 24) | (q(self.r) << 16) | (q(self.g) << 8) | q(self.b)
    }
}

pub const DEFAULT_TINT: RgbColor = RgbColor::hex(0x8B8E94);

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    let c = c.max(0.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_to_oklab(c: RgbColor) -> (f32, f32, f32) {
    let r = srgb_to_linear(c.r);
    let g = srgb_to_linear(c.g);
    let b = srgb_to_linear(c.b);
    let l = (0.41222146 * r + 0.53633255 * g + 0.051445995 * b).cbrt();
    let m = (0.2119035 * r + 0.6806995 * g + 0.10739696 * b).cbrt();
    let s = (0.08830246 * r + 0.28171885 * g + 0.6299787 * b).cbrt();
    (
        0.21045426 * l + 0.7936178 * m - 0.004072047 * s,
        1.9779985 * l - 2.4285922 * m + 0.4505937 * s,
        0.025904037 * l + 0.78277177 * m - 0.80867577 * s,
    )
}

fn oklab_to_srgb(l_ok: f32, a_ok: f32, b_ok: f32, alpha: f32) -> RgbColor {
    let l_ = l_ok + 0.39633778 * a_ok + 0.21580376 * b_ok;
    let m_ = l_ok - 0.105561346 * a_ok - 0.06385417 * b_ok;
    let s_ = l_ok - 0.08948418 * a_ok - 1.2914855 * b_ok;
    let (l, m, s) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.0767417 * l - 3.3077116 * m + 0.23096994 * s;
    let g = -1.268438 * l + 2.6097574 * m - 0.34131938 * s;
    let b = -0.0041960863 * l - 0.7034186 * m + 1.7076147 * s;
    RgbColor {
        r: linear_to_srgb(r).clamp(0.0, 1.0),
        g: linear_to_srgb(g).clamp(0.0, 1.0),
        b: linear_to_srgb(b).clamp(0.0, 1.0),
        a: alpha.clamp(0.0, 1.0),
    }
}

/// CSS `color-mix(in oklab, first first_pct%, second)`.
pub fn mix_oklab(first: RgbColor, first_pct: f32, second: RgbColor) -> RgbColor {
    let w1 = (first_pct / 100.0).clamp(0.0, 1.0);
    let w2 = 1.0 - w1;
    let (l1, a1, b1) = srgb_to_oklab(first);
    let (l2, a2, b2) = srgb_to_oklab(second);
    let alpha = first.a * w1 + second.a * w2;
    if alpha <= 1e-6 {
        return RgbColor { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    }
    let l = (l1 * first.a * w1 + l2 * second.a * w2) / alpha;
    let a = (a1 * first.a * w1 + a2 * second.a * w2) / alpha;
    let b = (b1 * first.a * w1 + b2 * second.a * w2) / alpha;
    oklab_to_srgb(l, a, b, alpha)
}

pub fn parse_hex_color(raw: &str) -> Option<RgbColor> {
    let s = raw.trim().trim_start_matches('#');
    match s.len() {
        6 => Some(RgbColor::hex(u32::from_str_radix(s, 16).ok()?)),
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).ok()?;
            let g = u8::from_str_radix(&s[1..2], 16).ok()?;
            let b = u8::from_str_radix(&s[2..3], 16).ok()?;
            Some(RgbColor::rgba_u8(r * 17, g * 17, b * 17, 1.0))
        }
        8 => {
            let v = u32::from_str_radix(&s[0..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()? as f32 / 255.0;
            Some(RgbColor::hex(v).with_alpha(a))
        }
        _ => None,
    }
}

/// Every surface/ink token of the Vively design language.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaletteTokens {
    pub is_dark: bool,
    pub tint: RgbColor,
    pub is_neutral: bool,
    pub base: RgbColor,
    pub ground: RgbColor,
    pub rail: RgbColor,
    pub raised: RgbColor,
    pub stage_bg: RgbColor,
    pub dock: RgbColor,
    pub fill: RgbColor,
    pub fill_2: RgbColor,
    pub hover: RgbColor,
    pub active: RgbColor,
    pub line: RgbColor,
    pub art_ring: RgbColor,
    pub ink: RgbColor,
    pub ink_2: RgbColor,
    pub ink_3: RgbColor,
    pub hi: RgbColor,
    pub cta_bg: RgbColor,
    pub cta_fg: RgbColor,
    pub cta_hover: RgbColor,
    pub live_dot: RgbColor,
    pub danger: RgbColor,
    pub scrim: RgbColor,
}

impl PaletteTokens {
    pub fn resolve(is_dark: bool, tint: RgbColor, is_neutral: bool) -> Self {
        let h = RgbColor::hex;
        let a = RgbColor::rgba_hex;
        if is_dark {
            let base = h(0x1C1D1F);
            let ink = h(0xF2F1EF);
            let hi = if is_neutral { ink } else { mix_oklab(tint, 62.0, h(0xFFFFFF)) };
            Self {
                is_dark,
                tint,
                is_neutral,
                base,
                ground: mix_oklab(tint, 9.0, base),
                rail: mix_oklab(tint, 6.0, h(0x161719)),
                raised: mix_oklab(tint, 8.0, h(0x26272A)),
                stage_bg: mix_oklab(tint, 32.0, h(0x161718)),
                dock: mix_oklab(tint, 30.0, h(0x242528)),
                fill: a(0xffffff12),
                fill_2: a(0xffffff1f),
                hover: a(0xffffff0d),
                active: a(0xffffff17),
                line: a(0xffffff14),
                art_ring: a(0xffffff14),
                ink,
                ink_2: h(0xA3A3A6),
                ink_3: h(0x6E6F73),
                hi,
                cta_bg: hi,
                cta_fg: h(0x141414),
                cta_hover: mix_oklab(hi, 80.0, h(0xFFFFFF)),
                live_dot: h(0x3DBE66),
                danger: h(0xF0645A),
                scrim: RgbColor::rgba_u8(8, 8, 10, 0.66),
            }
        } else {
            let base = h(0xF1F2F3);
            let ink = h(0x17181A);
            let hi = if is_neutral { ink } else { mix_oklab(tint, 72.0, h(0x000000)) };
            Self {
                is_dark,
                tint,
                is_neutral,
                base,
                ground: mix_oklab(tint, 4.0, base),
                rail: mix_oklab(tint, 6.0, h(0xE5E6E8)),
                raised: h(0xFBFBFB),
                stage_bg: mix_oklab(tint, 22.0, h(0xF1F1F1)),
                dock: mix_oklab(tint, 18.0, h(0xFFFFFF)),
                fill: a(0x0000000e),
                fill_2: a(0x0000001a),
                hover: a(0x0000000a),
                active: a(0x00000013),
                line: a(0x00000017),
                art_ring: a(0x00000014),
                ink,
                ink_2: h(0x5B5D62),
                ink_3: h(0x8C8E93),
                hi,
                cta_bg: hi,
                cta_fg: h(0xFFFFFF),
                cta_hover: mix_oklab(hi, 85.0, h(0x000000)),
                live_dot: h(0x1F9D4C),
                danger: h(0xC42B22),
                scrim: RgbColor::rgba_u8(20, 20, 24, 0.36),
            }
        }
    }

    pub fn hero_banner_bg(&self, col_tint: Option<RgbColor>) -> RgbColor {
        let pct = if self.is_dark { 22.0 } else { 16.0 };
        mix_oklab(col_tint.unwrap_or(self.tint), pct, self.ground)
    }

    pub fn wave_hero_bg(&self) -> RgbColor {
        mix_oklab(self.hi, 13.0, self.ground)
    }
}
