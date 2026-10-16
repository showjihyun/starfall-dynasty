//! Player contracts (design doc Phase 3 "Contracts", §11.8 "Betrayal"; Kharz: history = contracts).
//!
//! An employer posts a job with the reward held in escrow; a contractor accepts; the simulation
//! checks fulfilment itself. Turning on the other party while a contract stands is a betrayal: the
//! contract breaks, the escrow goes to the wronged side, and the chronicle remembers.

use crate::chronicle::EventId;
use crate::galaxy::{self, Ore, SystemId};
use serde::{Deserialize, Serialize};

pub type ContractId = u64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ContractKind {
    /// Destroy this captain's flagship.
    Hunt { target: String },
    /// Bring this much ore to the station of `system` (it goes to the employer's hold).
    Delivery { ore: Ore, units: u32, system: SystemId },
}

impl ContractKind {
    pub fn describe(&self) -> String {
        match self {
            ContractKind::Hunt { target } => format!("{target}의 함선 격침"),
            ContractKind::Delivery { ore, units, system } => format!("{} {units}단위를 {} 정거장으로 운송", ore.name(), galaxy::system(*system).name),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ContractState {
    Open,
    Accepted,
    Fulfilled,
    /// Broken by a betrayal; `by` turned on the other party.
    Breached { by: String },
    Expired,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    pub id: ContractId,
    pub employer: u64,
    pub employer_name: String,
    pub kind: ContractKind,
    /// Credits held in escrow.
    pub reward: u64,
    pub posted_unix: u64,
    pub expires_unix: u64,
    pub state: ContractState,
    pub contractor: Option<(u64, String)>,
    /// The chronicle entry of the signing (cause of what follows).
    pub signed_event: Option<EventId>,
}

impl Contract {
    /// Accepted and still running: the window in which turning on the other party is a betrayal.
    pub fn binding(&self) -> bool {
        self.state == ContractState::Accepted
    }

    pub fn parties(&self) -> Option<(u64, u64)> {
        self.contractor.as_ref().map(|(c, _)| (self.employer, *c))
    }
}
