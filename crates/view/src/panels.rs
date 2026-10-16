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

// ---- Contract board -------------------------------------------------------------------------

use shared::contracts::{Contract, ContractId, ContractKind, ContractState};
use shared::galaxy::{Ore, SystemId};

/// What the player asked the contract board for.
#[derive(Clone, Debug, PartialEq)]
pub enum ContractRequest {
    Post { kind: ContractKind, reward: u64, hours: u32 },
    Accept(ContractId),
    Cancel(ContractId),
    Deliver(ContractId),
}

/// The "new contract" form (kept by the client between frames).
#[derive(Clone, Debug, PartialEq)]
pub struct ContractForm {
    pub hunt: bool,
    pub target: String,
    pub ore: Ore,
    pub units: u32,
    pub system: SystemId,
    pub reward: u64,
    pub hours: u32,
}

impl Default for ContractForm {
    fn default() -> Self {
        ContractForm { hunt: true, target: String::new(), ore: Ore::Xenite, units: 10, system: 1, reward: 1000, hours: 24 }
    }
}

fn state_label(c: &Contract) -> (String, Color32) {
    match &c.state {
        ContractState::Open => ("모집 중".into(), Color32::WHITE),
        ContractState::Accepted => (format!("진행 중 — {}", c.contractor.as_ref().map(|c| c.1.as_str()).unwrap_or("?")), GOLD),
        ContractState::Fulfilled => ("완수".into(), crate::palette::GREEN),
        ContractState::Breached { by } => (format!("배신으로 파기 — {by}"), crate::palette::RED),
        ContractState::Expired => ("기한 만료".into(), DIM),
        ContractState::Cancelled => ("철회".into(), DIM),
    }
}

/// The contract board: open jobs to take, your own contracts, and a form to post one.
/// `me` is the viewer's player id; `credits` decides whether the post button is enabled.
pub fn contracts(ui: &mut egui::Ui, list: &[Contract], me: u64, credits: u64, form: &mut ContractForm) -> Option<ContractRequest> {
    let mut out = None;
    ui.label(RichText::new("계약 게시판").strong());
    ui.label(RichText::new("보상은 공증인이 보관합니다. 계약 중에 상대에게 총을 겨누면 배신 — 보상은 피해자에게, 배신은 연대기에.").size(11.0).color(DIM));
    if list.is_empty() {
        ui.label(RichText::new("게시된 계약이 없습니다.").color(DIM));
    }
    for c in list {
        let mine = c.employer == me;
        let taken = c.contractor.as_ref().is_some_and(|(p, _)| *p == me);
        let (state, col) = state_label(c);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("#{} {}", c.id, c.kind.describe())).strong());
            ui.label(RichText::new(format!("의뢰 {} · 보상 {} CR", c.employer_name, c.reward)).size(11.0));
            ui.label(RichText::new(state).size(11.0).color(col));
            match &c.state {
                ContractState::Open if mine => {
                    if ui.small_button("철회").clicked() {
                        out = Some(ContractRequest::Cancel(c.id));
                    }
                }
                ContractState::Open => {
                    if ui.small_button("수락").clicked() {
                        out = Some(ContractRequest::Accept(c.id));
                    }
                }
                ContractState::Accepted if taken && matches!(c.kind, ContractKind::Delivery { .. }) => {
                    if ui.small_button("납품").on_hover_text("목적지 정거장에 도킹한 상태에서 화물을 넘깁니다").clicked() {
                        out = Some(ContractRequest::Deliver(c.id));
                    }
                }
                _ => {}
            }
        });
    }
    ui.separator();
    ui.label(RichText::new("새 계약").strong());
    ui.horizontal(|ui| {
        ui.radio_value(&mut form.hunt, true, "사냥");
        ui.radio_value(&mut form.hunt, false, "운송");
    });
    if form.hunt {
        ui.horizontal(|ui| {
            ui.label("표적 함장");
            ui.add(egui::TextEdit::singleline(&mut form.target).desired_width(160.0));
        });
    } else {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("ore").selected_text(form.ore.name()).show_ui(ui, |ui| {
                for o in Ore::ALL {
                    ui.selectable_value(&mut form.ore, o, o.name());
                }
            });
            ui.add(egui::DragValue::new(&mut form.units).range(1..=500).suffix(" 단위"));
            egui::ComboBox::from_id_salt("dest").selected_text(shared::galaxy::system(form.system).name).show_ui(ui, |ui| {
                for s in shared::galaxy::SYSTEMS {
                    ui.selectable_value(&mut form.system, s.id, s.name);
                }
            });
        });
    }
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut form.reward).range(0..=1_000_000).suffix(" CR"));
        ui.add(egui::DragValue::new(&mut form.hours).range(1..=72).suffix(" 시간"));
        let kind = if form.hunt { ContractKind::Hunt { target: form.target.trim().to_string() } } else { ContractKind::Delivery { ore: form.ore, units: form.units, system: form.system } };
        let ok = credits >= form.reward && (!form.hunt || !form.target.trim().is_empty());
        if ui.add_enabled(ok, egui::Button::new("게시 (보상 공탁)")).clicked() {
            out = Some(ContractRequest::Post { kind, reward: form.reward, hours: form.hours });
        }
    });
    out
}
