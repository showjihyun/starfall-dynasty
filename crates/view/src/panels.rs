//! Window panels drawn from plain data (no Bevy, no network): they return what the player asked for
//! and the client sends it. Checked headless in `tests/panels.rs`.

use crate::palette::{DIM, GOLD};
use egui::{Color32, RichText};
use shared::artifacts::{Artifact, ArtifactId};

/// A station service the player clicked for an artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactRequest {
    Appraise(ArtifactId),
    Sell(ArtifactId),
    Donate(ArtifactId),
}

/// What the station would pay or charge, so buttons can show it before the click.
pub struct ArtifactPrices {
    /// Appraisal fee for this captain (0 for an Archaeologist).
    pub appraisal: u64,
    pub unappraised_sale: f32,
    pub donation_share: f32,
    pub donation_reputation: i32,
}

/// The station window's artifact section: each held artifact with its provenance and the three
/// services. Buttons work only while docked.
pub fn artifacts(ui: &mut egui::Ui, held: &[Artifact], docked: bool, prices: &ArtifactPrices) -> Option<ArtifactRequest> {
    let mut out = None;
    ui.label(RichText::new(format!("유물 {}점", held.len())).strong());
    if held.is_empty() {
        ui.label(RichText::new("유물 없음 — 고고학자는 유적·고대 게이트를 스캔(V)해 발굴합니다. 격침된 함선의 잔해에서 회수할 수도 있습니다.").color(DIM).size(12.0));
        return None;
    }
    for a in held {
        ui.group(|ui| {
            ui.label(RichText::new(a.label()).color(if a.appraised { GOLD } else { Color32::WHITE }).strong());
            let worth = if a.appraised { format!("{} 유물 · 감정가 {} CR", a.culture.name(), a.value) } else { "감정 전 — 이름과 가치를 알 수 없다".to_string() };
            ui.label(RichText::new(format!("{} · 출토지 {}", worth, a.origin_site)).size(11.0).color(DIM));
            ui.collapsing(format!("내력 ({})", a.provenance.len()), |ui| {
                for p in &a.provenance {
                    ui.label(RichText::new(format!("{}  {}", p.date, p.text)).size(11.0));
                }
            });
            ui.horizontal(|ui| {
                if !a.appraised {
                    let txt = if prices.appraisal == 0 { "감정 (무료)".to_string() } else { format!("감정 ({} CR)", prices.appraisal) };
                    if ui.add_enabled(docked, egui::Button::new(txt)).clicked() {
                        out = Some(ArtifactRequest::Appraise(a.id));
                    }
                }
                let sale = if a.appraised { format!("판매 ({} CR)", a.value) } else { format!("판매 (미감정 {:.0}%)", prices.unappraised_sale * 100.0) };
                if ui.add_enabled(docked, egui::Button::new(sale)).clicked() {
                    out = Some(ArtifactRequest::Sell(a.id));
                }
                let fee = if a.appraised { format!(", 사례금 {} CR", (a.value as f32 * prices.donation_share) as u64) } else { String::new() };
                if ui.add_enabled(docked, egui::Button::new(format!("박물관 기증 (평판 +{}{fee})", prices.donation_reputation))).clicked() {
                    out = Some(ArtifactRequest::Donate(a.id));
                }
            });
        });
    }
    out
}
