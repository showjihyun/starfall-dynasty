//! Player institutions (design doc §24 "Museums & Institutions"): a captain founds a research
//! institute or museum at a station; artifacts donated to it go on display with their provenance;
//! captains who dock in its system visit it, and the visits pay the founder.

use crate::artifacts::{Artifact, ArtifactId};
use crate::galaxy::SystemId;
use serde::{Deserialize, Serialize};

pub type InstituteId = u64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Institute {
    pub id: InstituteId,
    pub name: String,
    pub founder: u64,
    pub founder_name: String,
    pub system: SystemId,
    pub founded_unix: u64,
    /// Artifacts on display, in order of acquisition.
    pub exhibits: Vec<ArtifactId>,
    /// Distinct visits so far (one per captain per day).
    pub visitors: u32,
}

/// An institute with its exhibits resolved, as clients see it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstituteView {
    pub institute: Institute,
    pub exhibits: Vec<Artifact>,
}
