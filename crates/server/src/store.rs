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
}

#[derive(Default, Serialize, Deserialize)]
pub struct StoreData {
    pub next_id: PlayerId,
    pub accounts: BTreeMap<PlayerId, Account>,
    pub wrecks: Vec<StoredWreck>,
    /// Accumulated supply per (system, ore) — drives local prices.
    pub supply: Vec<(SystemId, Ore, f32)>,
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

pub fn new_token(seed: u64) -> String {
    let mut r = shared::rng::Rng::from_parts(&[seed, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)]);
    format!("{:016x}{:016x}", r.next_u64(), r.next_u64())
}
