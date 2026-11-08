//! Persistent player data: accounts, profiles, wrecks and market state as JSON snapshots.
//! History lives separately in the append-only event log (see the `history` crate).

use serde::{Deserialize, Serialize};
use shared::galaxy::{Ore, SystemId};
use shared::protocol::{PlayerId, Profile};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    pub id: PlayerId,
    pub name: String,
    pub token: String,
    pub profile: Option<Profile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredWreck {
    pub system: SystemId,
    pub pos: [f32; 3],
    pub victim: String,
    pub ship: String,
    pub event: u64,
    pub cargo: BTreeMap<String, u32>,
    pub created_unix: u64,
    pub investigated_by: Vec<String>,
    /// Artifacts that went down with the ship, waiting for whoever investigates.
    /// (Left out of the JSON while empty, so stores and digests from before artifacts are unchanged.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<u64>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct StoreData {
    pub next_id: PlayerId,
    pub accounts: BTreeMap<PlayerId, Account>,
    pub wrecks: Vec<StoredWreck>,
    /// Accumulated supply per (system, ore) — drives local prices.
    pub supply: Vec<(SystemId, Ore, f32)>,
    #[serde(default)]
    pub alliances: Vec<Alliance>,
    #[serde(default)]
    pub next_alliance: u32,
    /// Every artifact ever unearthed, wherever it is now. (The artifact fields are left out of the
    /// JSON while empty, so stores and digests from before artifacts are unchanged.)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub artifacts: BTreeMap<u64, shared::artifacts::Artifact>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_artifact: u64,
    /// (system, poi, day) sites already excavated: one artifact per site per UTC day.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excavated: Vec<(SystemId, u32, u32)>,
    /// Herald article id → the distinct readers its reporter has been paid for.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub story_readers: BTreeMap<u64, Vec<PlayerId>>,
    /// Every contract ever posted (finished ones stay as records).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub contracts: BTreeMap<u64, shared::contracts::Contract>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_contract: u64,
    /// Political influence: system → alliance → points.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub influence: BTreeMap<SystemId, BTreeMap<u32, f32>>,
    /// System → controlling alliance.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub controllers: BTreeMap<SystemId, u32>,
    /// Station batteries that are fortified, worn down or silenced (`social.station_guard`); a
    /// system missing here has fresh batteries at level 0.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub batteries: BTreeMap<SystemId, Battery>,
    /// Captains who left an alliance at one of its stations: (system, influence carried, unix
    /// deadline) for the next alliance they found or join (`social.betrayal.defect_share`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub defectors: BTreeMap<PlayerId, (SystemId, f32, u64)>,
    /// Who has blocked whom: captain → the captains whose chat they do not receive (`/block`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub blocks: BTreeMap<PlayerId, std::collections::BTreeSet<PlayerId>>,
    /// Player reports for the operator (`/report`, console `reports`), oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reports: Vec<Report>,
    /// Captain names (lowercase) the operator banned (`ban` on the server console).
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub banned: std::collections::BTreeSet<String>,
    /// Battles fought between two alliances (lower id first): rivals at `social.rivalry_battles`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub battles_between: BTreeMap<(u32, u32), u32>,
    /// Player institutions (research institutes / museums).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub institutes: BTreeMap<u64, shared::institutes::Institute>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_institute: u64,
    /// Institute → (visitor, day) visits of the current day.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub institute_visits: BTreeMap<u64, Vec<(PlayerId, u32)>>,
}

/// A captain's report of another, with what the reported captain last said.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub unix: u64,
    pub reporter: String,
    pub target: String,
    pub reason: String,
    pub lines: Vec<String>,
}

/// A controlled system's station batteries. Belong to `owner`: when the system changes hands the
/// new holder starts with fresh ones.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Battery {
    pub owner: u32,
    pub level: u8,
    /// Integrity lost to the siege.
    pub damage: f32,
    pub silenced: bool,
    /// Seconds since an enemy was last on the approach (rebuilding silenced batteries).
    pub quiet: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alliance {
    pub id: u32,
    pub name: String,
    pub tag: String,
    pub leader: PlayerId,
    pub members: Vec<PlayerId>,
    /// Alliances this one is at war with (kept symmetric).
    pub wars: Vec<u32>,
    /// Tax collected in the systems it controls.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub treasury: u64,
    /// Broken treaties (`social.betrayal`).
    #[serde(default, skip_serializing_if = "is_zero_f")]
    pub infamy: f32,
    /// (alliance, unix end) truces after peace.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub truces: Vec<(u32, u64)>,
    /// Member → the member they back as leader.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub votes: BTreeMap<PlayerId, PlayerId>,
}

pub struct Store {
    pub data: StoreData,
    by_name: HashMap<String, PlayerId>,
    path: Option<PathBuf>,
    pub dirty: bool,
}

impl Store {
    pub fn memory() -> Self {
        Store { data: StoreData { next_id: 1, ..Default::default() }, by_name: HashMap::new(), path: None, dirty: false }
    }

    /// In-memory store over existing data (nothing is ever written).
    pub fn from_data(data: StoreData) -> Self {
        let by_name = data.accounts.values().map(|a| (a.name.to_lowercase(), a.id)).collect();
        Store { data, by_name, path: None, dirty: false }
    }

    pub fn open(path: PathBuf) -> std::io::Result<Self> {
        let data: StoreData = if path.exists() {
            serde_json::from_str(&std::fs::read_to_string(&path)?).map_err(std::io::Error::other)?
        } else {
            StoreData { next_id: 1, ..Default::default() }
        };
        let by_name = data.accounts.values().map(|a| (a.name.to_lowercase(), a.id)).collect();
        Ok(Store { data, by_name, path: Some(path), dirty: false })
    }

    /// Atomic save: write to a temp file, flush it to disk, then rename over the old snapshot (a crash
    /// or power cut leaves the old world or the new one, never half of one).
    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(p) = &self.path {
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let tmp = p.with_extension("json.tmp");
            {
                use std::io::Write;
                let mut f = std::fs::File::create(&tmp)?;
                f.write_all(serde_json::to_string(&self.data).map_err(std::io::Error::other)?.as_bytes())?;
                f.sync_all()?;
            }
            std::fs::rename(&tmp, p)?;
        }
        self.dirty = false;
        Ok(())
    }

    /// Copy the saved world to `backups/world_<unix>.json` beside it and keep the newest `keep` copies
    /// (an operator can roll back a day of play after a bad bug). Returns the copy written, if any.
    pub fn backup(&self, unix: u64, keep: usize) -> std::io::Result<Option<PathBuf>> {
        let Some(p) = self.path.as_ref().filter(|p| p.exists()) else { return Ok(None) };
        let dir = p.parent().map(|d| d.join("backups")).unwrap_or_else(|| PathBuf::from("backups"));
        std::fs::create_dir_all(&dir)?;
        let to = dir.join(format!("world_{unix}.json"));
        std::fs::copy(p, &to)?;
        let mut all: Vec<PathBuf> = std::fs::read_dir(&dir)?.filter_map(Result::ok).map(|e| e.path()).filter(|f| f.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("world_") && n.ends_with(".json"))).collect();
        all.sort();
        while all.len() > keep {
            let _ = std::fs::remove_file(all.remove(0));
        }
        Ok(Some(to))
    }

    pub fn find_by_name(&self, name: &str) -> Option<PlayerId> {
        self.by_name.get(&name.to_lowercase()).copied()
    }

    pub fn create_account(&mut self, name: &str, token: String) -> PlayerId {
        let id = self.data.next_id;
        self.data.next_id += 1;
        self.data.accounts.insert(id, Account { id, name: name.to_string(), token, profile: None });
        self.by_name.insert(name.to_lowercase(), id);
        self.dirty = true;
        id
    }

    pub fn account(&self, id: PlayerId) -> Option<&Account> {
        self.data.accounts.get(&id)
    }

    pub fn profile(&self, id: PlayerId) -> Option<&Profile> {
        self.data.accounts.get(&id).and_then(|a| a.profile.as_ref())
    }

    pub fn profile_mut(&mut self, id: PlayerId) -> Option<&mut Profile> {
        self.dirty = true;
        self.data.accounts.get_mut(&id).and_then(|a| a.profile.as_mut())
    }

    pub fn set_profile(&mut self, id: PlayerId, p: Profile) {
        if let Some(a) = self.data.accounts.get_mut(&id) {
            a.profile = Some(p);
            self.dirty = true;
        }
    }

    pub fn supply(&self, sys: SystemId, ore: Ore) -> f32 {
        self.data.supply.iter().find(|(s, o, _)| *s == sys && *o == ore).map(|t| t.2).unwrap_or(0.0)
    }

    pub fn add_supply(&mut self, sys: SystemId, ore: Ore, amount: f32) {
        if let Some(t) = self.data.supply.iter_mut().find(|(s, o, _)| *s == sys && *o == ore) {
            t.2 = (t.2 + amount).max(0.0);
        } else {
            self.data.supply.push((sys, ore, amount.max(0.0)));
        }
        self.dirty = true;
    }

    /// Market recovers over time: supply decays toward zero.
    pub fn decay_supply(&mut self, factor: f32) {
        for t in &mut self.data.supply {
            t.2 *= factor;
        }
    }
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

fn is_zero_f(v: &f32) -> bool {
    *v == 0.0
}

/// 256-bit login token from the OS CSPRNG.
pub fn new_token() -> String {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).expect("OS random source unavailable");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Constant-time comparison so token checks don't leak timing.
pub fn token_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
