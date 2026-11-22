use vively_app::palette::{DEFAULT_TINT, PaletteTokens, RgbColor, mix_oklab, parse_hex_color};

fn close(a: RgbColor, b: RgbColor) -> bool {
    (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01 && (a.a - b.a).abs() < 0.01
}

#[test]
fn oklab_mix_endpoints_and_hex_parsing() {
    let red = RgbColor::hex(0xFF0000);
    let blue = RgbColor::hex(0x0000FF);
    assert!(close(mix_oklab(red, 100.0, blue), red));
    assert!(close(mix_oklab(red, 0.0, blue), blue));
    assert!(close(parse_hex_color("#f00").unwrap(), red));
    assert_eq!(parse_hex_color("#ff000080").unwrap().a, 128.0 / 255.0);
    assert!(parse_hex_color("nope").is_none());
    assert_eq!(RgbColor::rgba_hex(0xffffff12).argb(), 0x12ffffff);
}

#[test]
fn neutral_tint_highlights_with_ink_and_modes_differ() {
    let dark = PaletteTokens::resolve(true, DEFAULT_TINT, true);
    let light = PaletteTokens::resolve(false, DEFAULT_TINT, true);
    assert_eq!(dark.hi, dark.ink);
    assert_eq!(light.hi, light.ink);
    assert!(dark.ground.r < 0.3 && light.ground.r > 0.8);
    let tinted = PaletteTokens::resolve(true, RgbColor::hex(0xE04040), false);
    assert!(tinted.hi.r > tinted.hi.b, "tint leaks into highlight");
}
