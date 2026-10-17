//! The game's colours, one source of truth (docs/ART_DIRECTION.md, "색"). UI colours are sRGB
//! `Color32`; [`side`] holds the linear RGB the 3D materials and effects use for friend and foe.

use egui::Color32;

pub const ACCENT: Color32 = Color32::from_rgb(110, 190, 255);
pub const GOLD: Color32 = Color32::from_rgb(255, 205, 110);
pub const DIM: Color32 = Color32::from_rgb(140, 150, 170);
pub const RED: Color32 = Color32::from_rgb(255, 95, 80);
pub const GREEN: Color32 = Color32::from_rgb(110, 230, 150);
/// The part of a gauge that was just lost, glowing while it drains.
pub const LOSS: Color32 = Color32::from_rgb(255, 170, 60);
/// Shield blue: the shield gauge, bars and readouts.
pub const SHIELD: Color32 = Color32::from_rgb(80, 160, 255);
/// Points of interest not yet scanned.
pub const POI: Color32 = Color32::from_rgb(130, 230, 255);

/// Risk tier, from safe green to deep-space violet.
pub fn tier_color(t: shared::galaxy::RiskTier) -> Color32 {
    use shared::galaxy::RiskTier;
    match t {
        RiskTier::Core => GREEN,
        RiskTier::Frontier => Color32::from_rgb(255, 210, 90),
        RiskTier::Wild => Color32::from_rgb(255, 140, 60),
        RiskTier::WarZone => RED,
        RiskTier::Deep => Color32::from_rgb(190, 120, 255),
    }
}

/// Mineral rarity: grey, white, blue, violet, gold (rising value).
pub fn rarity_color(r: shared::minerals::Rarity) -> Color32 {
    use shared::minerals::Rarity;
    match r {
        Rarity::Common => DIM,
        Rarity::Uncommon => Color32::from_rgb(215, 222, 235),
        Rarity::Rare => Color32::from_rgb(120, 190, 255),
        Rarity::Exotic => Color32::from_rgb(200, 140, 255),
        Rarity::Legendary => GOLD,
    }
}

/// Friend and foe in 3D (linear RGB; materials apply their own emissive strength). Every glow,
/// plume, shield and shot of a ship takes its side's colours, decided by its relation to me
/// (`relation::Relation::Hostile` → `HOSTILE`), so a hostile captain reads red like a pirate.
pub mod side {
    pub struct Side {
        pub glow: [f32; 3],
        pub engine: [f32; 3],
        pub engine_core: [f32; 3],
        pub shield: [f32; 3],
        pub laser: [f32; 3],
        pub pulse: [f32; 3],
        pub beam: [f32; 3],
    }

    pub const FRIEND: Side = Side { glow: [0.45, 0.8, 1.0], engine: [0.3, 0.6, 1.0], engine_core: [0.85, 0.95, 1.0], shield: [0.35, 0.7, 1.0], laser: [0.25, 0.75, 1.0], pulse: [1.0, 0.75, 0.3], beam: [0.55, 0.8, 1.0] };
    pub const HOSTILE: Side = Side { glow: [1.0, 0.25, 0.15], engine: [1.0, 0.3, 0.1], engine_core: [1.0, 0.8, 0.5], shield: [1.0, 0.35, 0.2], laser: [1.0, 0.25, 0.12], pulse: [1.0, 0.3, 0.15], beam: [1.0, 0.45, 0.3] };

    pub fn of(hostile: bool) -> &'static Side {
        if hostile { &HOSTILE } else { &FRIEND }
    }
}

/// Species styles for the hull shader (docs/ART_DIRECTION.md §4): `[style, r, g, b]`, linear RGB.
/// Style 0 = Human, the baseline worn-industrial look (no extra treatment); 1 = Veyr, nacre shell
/// with violet veins (the rgb) shading to teal at the rim; 2 = Kharz, blackened iron with amber heat
/// seams (amber, never red: red means hostile). NPCs and unknown species get −1 (untouched).
pub fn species_style(s: Option<shared::galaxy::Species>) -> [f32; 4] {
    use shared::galaxy::Species;
    match s {
        Some(Species::Human) => [0.0, 0.85, 0.9, 1.0],
        Some(Species::Veyr) => [1.0, 0.6, 0.25, 1.0],
        Some(Species::Kharz) => [2.0, 1.0, 0.58, 0.18],
        None => [-1.0, 0.0, 0.0, 0.0],
    }
}
