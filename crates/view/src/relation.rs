//! How an entity relates to the viewer: one rule for the HUD brackets and the star map.

use crate::palette::{ACCENT, GOLD, GREEN, RED};
use egui::Color32;
use shared::galaxy::RiskTier;
use shared::protocol::{AllianceId, AllianceInfo, EntityId, EntityKind, EntityState, FleetId, PlayerId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Mine,
    Wreck,
    /// My escorts, fleet mates and alliance members.
    Friend,
    /// Pirates, and captains I may fight here (PvP systems, or an alliance war outside Core space).
    Hostile,
    Neutral,
}

impl Relation {
    pub fn color(self) -> Color32 {
        match self {
            Relation::Mine | Relation::Wreck => GOLD,
            Relation::Friend => GREEN,
            Relation::Hostile => RED,
            Relation::Neutral => ACCENT,
        }
    }
}

/// Who is looking, and where.
pub struct Viewer<'a> {
    pub me: Option<EntityId>,
    pub player: Option<PlayerId>,
    pub fleet: Option<FleetId>,
    pub alliance: Option<AllianceId>,
    pub alliances: &'a [AllianceInfo],
    pub tier: RiskTier,
}

impl Viewer<'_> {
    pub fn relation(&self, e: &EntityState) -> Relation {
        if e.kind == EntityKind::Wreck {
            return Relation::Wreck;
        }
        if Some(e.id) == self.me {
            return Relation::Mine;
        }
        let friend = e.owner.is_some() && (e.owner == self.player || (e.fleet.is_some() && e.fleet == self.fleet) || (e.alliance.is_some() && e.alliance == self.alliance));
        if friend {
            return Relation::Friend;
        }
        let war = e.alliance.is_some_and(|a| self.alliances.iter().any(|x| Some(x.id) == self.alliance && x.wars.contains(&a)));
        if matches!(e.kind, EntityKind::Npc(_)) || (e.owner.is_some() && (self.tier.pvp_allowed() || (war && self.tier != RiskTier::Core))) {
            Relation::Hostile
        } else {
            Relation::Neutral
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interp::tests::ent;
    use shared::missions::NpcArchetype;

    fn captain(id: EntityId, owner: PlayerId) -> EntityState {
        EntityState { owner: Some(owner), ..ent(id, [0.0; 3], [0.0; 3]) }
    }

    #[test]
    fn relations_follow_ownership_fleet_alliance_and_space() {
        let wars = [AllianceInfo { id: 1, name: "A".into(), tag: "A".into(), leader: "x".into(), members: vec![], wars: vec![2] }];
        let v = |tier| Viewer { me: Some(10), player: Some(7), fleet: Some(3), alliance: Some(1), alliances: &wars, tier };
        let core = v(RiskTier::Core);
        assert_eq!(core.relation(&captain(10, 7)), Relation::Mine);
        assert_eq!(core.relation(&captain(11, 7)), Relation::Friend, "my escort");
        assert_eq!(core.relation(&EntityState { fleet: Some(3), ..captain(12, 8) }), Relation::Friend);
        assert_eq!(core.relation(&EntityState { alliance: Some(1), ..captain(13, 9) }), Relation::Friend);
        assert_eq!(core.relation(&EntityState { kind: EntityKind::Npc(NpcArchetype::Raider), ..ent(14, [0.0; 3], [0.0; 3]) }), Relation::Hostile);
        assert_eq!(core.relation(&EntityState { kind: EntityKind::Wreck, ..captain(15, 7) }), Relation::Wreck);
        let enemy = EntityState { alliance: Some(2), ..captain(16, 9) };
        assert_eq!(core.relation(&enemy), Relation::Neutral, "no war in Core space");
        assert_eq!(v(RiskTier::Frontier).relation(&enemy), Relation::Hostile, "alliance war outside Core");
        assert_eq!(v(RiskTier::Frontier).relation(&captain(17, 9)), Relation::Neutral);
        assert_eq!(v(RiskTier::Wild).relation(&captain(17, 9)), Relation::Hostile, "open PvP");
        assert_eq!(v(RiskTier::Wild).relation(&ent(18, [0.0; 3], [0.0; 3])), Relation::Neutral, "ownerless props");
    }
}
