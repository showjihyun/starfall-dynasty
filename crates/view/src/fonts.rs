//! Korean UI font: the first system font found (Malgun Gothic on Windows, Nanum Gothic on Linux,
//! Apple SD Gothic Neo on macOS), behind egui's defaults for both families. Shared by the client and
//! the headless tests that measure Korean text.

pub const KOREAN_FONTS: [&str; 3] = ["C:/Windows/Fonts/malgun.ttf", "/usr/share/fonts/truetype/nanum/NanumGothic.ttf", "/System/Library/Fonts/AppleSDGothicNeo.ttc"];

/// egui's default fonts plus the Korean one; `None` for the path when no Korean font was found.
pub fn with_korean() -> (egui::FontDefinitions, Option<&'static str>) {
    let mut fonts = egui::FontDefinitions::default();
    for path in KOREAN_FONTS {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert("kr".into(), egui::FontData::from_owned(bytes).into());
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(fam).or_default().push("kr".into());
            }
            return (fonts, Some(path));
        }
    }
    (fonts, None)
}
