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
/// Caution: hull between 30% and 60%.
pub const CAUTION: Color32 = Color32::from_rgb(255, 200, 80);
/// Major chronicle events (historic ones are `GOLD`).
pub const MAJOR: Color32 = Color32::from_rgb(255, 160, 120);

/// `c` at opacity `a` (rings, fades and pulses of a token colour).
pub fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Hull colour, everywhere a hull is shown (crosshair gauge, status and target bars): `GREEN` above
/// 60%, `CAUTION` above 30%, then `RED` pulsing darker with `pulse` (1 = steady).
pub fn hull_color(hull: f32, pulse: f32) -> Color32 {
    if hull > 0.6 {
        GREEN
    } else if hull > 0.3 {
        CAUTION
    } else {
        Color32::from_rgb(RED.r(), (RED.g() as f32 * pulse) as u8, (RED.b() as f32 * pulse) as u8)
    }
}

/// Interface chrome: dark navy panels and frames that carry no meaning of their own.
pub mod chrome {
    use egui::Color32;
    pub const WINDOW: Color32 = Color32::from_rgba_unmultiplied_const(12, 16, 26, 235);
    pub const PANEL: Color32 = Color32::from_rgba_unmultiplied_const(12, 16, 26, 220);
    pub const BORDER: Color32 = Color32::from_rgb(50, 80, 120);
    pub const SELECTION: Color32 = Color32::from_rgb(40, 90, 150);
    /// Full-screen dimming behind character creation.
    pub const SCRIM: Color32 = Color32::from_rgba_unmultiplied_const(5, 8, 14, 170);
    /// Behind toasts and chat lines (alpha set per line).
    pub const INK: Color32 = Color32::from_rgb(10, 14, 24);
    /// Toast text.
    pub const PAPER: Color32 = Color32::from_rgb(230, 240, 255);
    /// Bar and gauge tracks, bar tick marks, the swatch dot.
    pub const TRACK: Color32 = Color32::from_rgba_unmultiplied_const(20, 25, 35, 200);
    pub const GAUGE_TRACK: Color32 = Color32::from_rgba_unmultiplied_const(15, 20, 30, 150);
    pub const TICK: Color32 = Color32::from_rgba_unmultiplied_const(0, 0, 0, 90);
    pub const DOT: Color32 = Color32::from_rgb(10, 12, 18);
    /// Track behind the small shield/hull bars over ships.
    pub const MINI_TRACK: Color32 = Color32::from_gray(40);
    pub const CROSSHAIR: Color32 = Color32::from_rgba_unmultiplied_const(170, 220, 255, 160);
    /// Danger popups (under attack) and the death screen.
    pub const DANGER: Color32 = Color32::from_rgba_unmultiplied_const(90, 20, 20, 220);
    pub const DEATH: Color32 = Color32::from_rgba_unmultiplied_const(40, 5, 5, 230);
    /// Damage vignette at the screen edge (alpha set per ring).
    pub const WOUND: Color32 = Color32::from_rgb(255, 30, 20);
}

/// Cinematics (view::cinematic, docs/STORY.md): letterbox, fades, subtitles, title card.
pub mod cinema {
    use egui::Color32;
    /// Letterbox bars and the black of a cut (alpha set per frame).
    pub const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
    pub const LINE: Color32 = Color32::from_rgb(235, 240, 250);
    /// Who speaks, under the line.
    pub const SOURCE: Color32 = Color32::from_rgb(150, 165, 190);
    /// Soft drop shadow behind subtitles so they read over bright stars and explosions.
    pub const SHADOW: Color32 = Color32::from_rgba_unmultiplied_const(0, 0, 0, 200);
    pub const TITLE: Color32 = Color32::from_rgb(235, 240, 250);
    pub const CHAPTER: Color32 = Color32::from_rgb(255, 205, 110);
}

/// Star map (galaxy and system charts).
pub mod map {
    use egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(4, 7, 13);
    pub const GRID: Color32 = Color32::from_rgb(22, 34, 52);
    pub const GRID_TEXT: Color32 = Color32::from_rgb(70, 95, 130);
    pub const LANE: Color32 = Color32::from_rgb(34, 52, 80);
    pub const ORBIT: Color32 = Color32::from_rgba_unmultiplied_const(80, 110, 160, 40);
    pub const PLANET: Color32 = Color32::from_rgb(110, 120, 150);
    pub const PLANET_RIM: Color32 = Color32::from_rgb(170, 180, 210);
    pub const ASTEROID: Color32 = Color32::from_rgb(90, 80, 72);
    /// My reach: main weapons (warm red) and turrets (amber, like turret fire).
    pub const WEAPON_REACH: Color32 = Color32::from_rgb(255, 120, 90);
    pub const TURRET_REACH: Color32 = Color32::from_rgb(255, 180, 90);
    pub const SIDEBAR: Color32 = Color32::from_rgb(8, 12, 20);
    pub const TOOLTIP: Color32 = Color32::from_rgba_unmultiplied_const(8, 12, 20, 235);
    pub const TOOLTIP_BORDER: Color32 = Color32::from_rgb(40, 60, 90);
    pub const DIVIDER: Color32 = Color32::from_rgb(30, 45, 70);
}

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

/// Hull paint (linear RGB, multiplied into the livery plates; docs/ART_DIRECTION.md §3.6). The first
/// [`PLAYER_LIVERIES`] are the captain's choice; the last is the pirates' red.
pub const LIVERIES: [[f32; 3]; 8] = [
    [0.85, 0.85, 0.88], // titanium
    [0.15, 0.45, 0.95], // navy blue
    [0.95, 0.55, 0.1],  // amber
    [0.1, 0.75, 0.55],  // jade
    [0.75, 0.15, 0.2],  // crimson
    [0.55, 0.3, 0.9],   // violet
    [0.95, 0.85, 0.2],  // gold
    [0.8, 0.12, 0.1],   // hostile red
];
pub const PLAYER_LIVERIES: usize = 7;
pub const HOSTILE_LIVERY: u8 = 7;

/// Each species' default paint at character creation (§4): Human navy (gunmetal and navy), Veyr violet
/// (its nacre and veins), Kharz gold (brass clan fittings — not red, which means hostile).
pub fn species_livery(s: shared::galaxy::Species) -> u8 {
    use shared::galaxy::Species;
    match s {
        Species::Human => 1,
        Species::Veyr => 5,
        Species::Kharz => 6,
    }
}

/// The ancient civilisation's light (gates, ruins, glyphs; docs/ART_DIRECTION.md §4): teal-cyan, linear
/// RGB, and the emissive strength that keeps its hue instead of blowing out to white.
pub const ANCIENT: [f32; 3] = [0.2, 0.9, 0.8];
pub const ANCIENT_GLOW: f32 = 2.2;
