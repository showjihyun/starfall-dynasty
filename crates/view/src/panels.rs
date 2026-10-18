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

/// 3D combat in two lines (docs/3D_전투_설계.md): what beats what. Fleet window and help (F1).
pub const TACTICS_3D: [&str; 2] = [
    "3D 전술: 순양함·주력함의 포탑은 바로 위·아래로는 절반만 닿는다 — 혼자라면 위·아래로 파고들어라.",
    "방어: 구르기(Q/E)로 적을 선체 옆에 두면 포탑이 모두 닿는다. 함대는 공격 각도를 나누면(위 + 옆) 구르기를 이긴다.",
];

/// The fleet window's footer: what a fleet shares, and how to attack together in 3D.
pub fn fleet_tips(ui: &mut egui::Ui) {
    ui.label(RichText::new("함대원끼리는 서로 피해를 주지 않고, 격파 보상과 미션 진행을 공유합니다. War Zone에서 함대 대 함대 교전은 연대기에 전투로 기록됩니다.").size(11.0).color(DIM));
    for t in TACTICS_3D {
        ui.label(RichText::new(t).size(11.0).color(DIM));
    }
}

/// The station window's mineral specimens: each catalogue mineral held with its rarity, best
/// properties and price, and one button to sell them all (docked only). True when it was clicked.
pub fn specimens(ui: &mut egui::Ui, held: &std::collections::BTreeMap<shared::minerals::MineralId, u32>, docked: bool) -> bool {
    use shared::minerals::{self, Property};
    let t = shared::tuning::get();
    ui.label(RichText::new("광물 표본").strong());
    if held.is_empty() {
        let height = if t.minerals.height_bonus.weight > 0.0 { " 희귀 광맥은 정거장에서 위아래로 멀리 오르내린 소행성에 많습니다(지도에서 소행성에 마우스를 올려 확인)." } else { "" };
        ui.label(RichText::new(format!("표본 없음 — 희귀 이상 광맥을 캐면 가끔 그 광물의 표본이 나옵니다.{height} 처음 캐낸 광물은 연대기에 발견자로 남습니다.")).size(11.0).color(DIM));
        return false;
    }
    let mut total = 0;
    for (id, n) in held {
        let Some(m) = minerals::mineral(*id) else { continue };
        let each = m.specimen_value(&t.minerals);
        total += each * *n as u64;
        let mut best: Vec<Property> = Property::ALL.to_vec();
        best.sort_by_key(|p| std::cmp::Reverse(m.prop(*p)));
        let traits: Vec<String> = best.iter().take(2).map(|p| format!("{} {}", p.label(), m.prop(*p))).collect();
        let col = crate::palette::rarity_color(m.rarity);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{} × {n}", m.label())).color(col));
            ui.label(RichText::new(format!("{} · {each} CR/점", traits.join(", "))).size(11.0).color(DIM));
        });
    }
    ui.add_enabled(docked, egui::Button::new(format!("표본 전량 판매 (+{total} CR)"))).clicked()
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

/// Systems a delivery can be posted to: the ones the map names (no leaking uncharted names).
pub fn delivery_destinations(charted: &std::collections::BTreeSet<shared::galaxy::SystemId>, here: shared::galaxy::SystemId) -> Vec<&'static shared::galaxy::SystemDef> {
    shared::galaxy::systems().iter().filter(|s| crate::mapview::known(s.id, charted, here)).collect()
}

/// The contract board: open jobs to take, your own contracts, and a form to post one.
/// `me` is the viewer's player id; `credits` decides whether the post button is enabled; delivery
/// destinations are the systems the map names (`charted` generated ones, see `mapview::known`).
pub fn contracts(ui: &mut egui::Ui, list: &[Contract], me: u64, credits: u64, charted: &std::collections::BTreeSet<shared::galaxy::SystemId>, here: shared::galaxy::SystemId, form: &mut ContractForm) -> Option<ContractRequest> {
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
                for s in delivery_destinations(charted, here) {
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

// ---- Political influence ----------------------------------------------------------------------

use shared::protocol::SystemStanding;

/// Influence standings per system: who holds it and the top contenders, with `mine` (my alliance
/// tag) highlighted. Systems nobody has any influence in are left out.
pub fn influence(ui: &mut egui::Ui, systems: &[SystemStanding], mine: Option<&str>, threshold: f32) {
    ui.label(RichText::new("성계 영향력").strong());
    ui.label(RichText::new(format!("동맹원이 성계에서 해적 소탕·광석 판매·계약 완수·박물관 기증을 하고 교전에서 이기면 영향력이 쌓입니다(지면 줄어듭니다). {threshold:.0} 이상이고 지배 세력보다 20% 앞서면 지배권을 가져옵니다. 지배 동맹원은 그 성계에서 현상금·판매가 +10%, 다른 함장의 광석 판매·현상금 5%는 지배 동맹 금고로 갑니다. 많이 쥘수록 새 성계 영향력은 덜 쌓이고, 둘째 성계부터 금고에서 시간당 유지비가 나갑니다(못 내면 가장 약한 성계부터 철수). 영향력은 매시간 줄어듭니다. Core 성계는 지배할 수 없습니다.")).size(11.0).color(DIM));
    let active: Vec<&SystemStanding> = systems.iter().filter(|s| s.controller.is_some() || !s.standings.is_empty()).collect();
    if active.is_empty() {
        ui.label(RichText::new("아직 어느 성계에도 영향력을 가진 동맹이 없습니다.").color(DIM));
        return;
    }
    egui::Grid::new("influence").striped(true).num_columns(3).show(ui, |ui| {
        for s in active {
            ui.label(RichText::new(shared::galaxy::system(s.system).name).strong());
            match &s.controller {
                Some((_, tag)) => ui.label(RichText::new(format!("지배 [{tag}]")).color(if Some(tag.as_str()) == mine { crate::palette::GREEN } else { GOLD })),
                None => ui.label(RichText::new("무주지").color(DIM)),
            };
            let top: Vec<String> = s.standings.iter().take(3).map(|(_, tag, v)| format!("[{tag}] {v:.0}")).collect();
            ui.label(RichText::new(top.join(" · ")).size(11.0));
            ui.end_row();
        }
    });
}

// ---- Institutes ---------------------------------------------------------------------------------

use shared::institutes::{InstituteId, InstituteView};

#[derive(Clone, Debug, PartialEq)]
pub enum InstituteRequest {
    Found(String),
    Donate { artifact: u64, institute: InstituteId },
}

/// The station's institutions: what is on display here (with each artifact's provenance), buttons to
/// donate the artifacts I hold, and a form to found one if I have none yet.
pub fn institutes(ui: &mut egui::Ui, list: &[InstituteView], here: SystemId, held: &[Artifact], me: u64, credits: u64, docked: bool, cost: u64, name: &mut String) -> Option<InstituteRequest> {
    let mut out = None;
    let local: Vec<&InstituteView> = list.iter().filter(|v| v.institute.system == here).collect();
    ui.label(RichText::new(format!("연구소·박물관 {}곳", local.len())).strong());
    if local.is_empty() {
        ui.label(RichText::new("이 정거장에는 아직 기관이 없습니다. 기관을 세우고 유물을 전시하면, 이 성계에 도킹하는 함장들이 관람객이 되어 수입이 생깁니다.").size(11.0).color(DIM));
    }
    for v in &local {
        let i = &v.institute;
        ui.group(|ui| {
            ui.label(RichText::new(&i.name).color(GOLD).strong());
            ui.label(RichText::new(format!("설립 {} · 전시 {}점 · 관람 {}회", i.founder_name, i.exhibits.len(), i.visitors)).size(11.0).color(DIM));
            for a in &v.exhibits {
                ui.collapsing(format!("{} — {}", a.label(), a.culture.name()), |ui| {
                    for p in &a.provenance {
                        ui.label(RichText::new(format!("{}  {}", p.date, p.text)).size(11.0));
                    }
                });
            }
            for a in held {
                if ui.add_enabled(docked, egui::Button::new(format!("{} 기증", a.label()))).clicked() {
                    out = Some(InstituteRequest::Donate { artifact: a.id, institute: i.id });
                }
            }
        });
    }
    let others = list.len() - local.len();
    if others > 0 {
        ui.label(RichText::new(format!("다른 성계의 기관 {others}곳")).size(11.0).color(DIM));
    }
    if !list.iter().any(|v| v.institute.founder == me) {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(name).hint_text("기관 이름").desired_width(220.0));
            let ok = docked && credits >= cost && name.trim().chars().count() >= 2;
            if ui.add_enabled(ok, egui::Button::new(format!("설립 ({cost} CR)"))).clicked() {
                out = Some(InstituteRequest::Found(name.trim().to_string()));
            }
        });
    }
    out
}
