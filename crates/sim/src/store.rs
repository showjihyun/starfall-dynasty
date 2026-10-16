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

    /// Atomic save: write to a temp file then rename over the old snapshot.
    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(p) = &self.path {
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let tmp = p.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_string(&self.data).map_err(std::io::Error::other)?)?;
            std::fs::rename(&tmp, p)?;
        }
        self.dirty = false;
        Ok(())
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
