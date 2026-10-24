//! Player alliance events: ToggleAlliance7318F0, CanAlly501540,
//! MakeAlly4F9B70 and MakeEnemy4F9F90 in the installed gamemd.exe.
//! Launch/map alliances continue through the existing graph/threat owner.
//! Offline scope: AI team edges cannot be changed through player input. The
//! human-to-human path is retained for command/replay tests; multiplayer-only
//! Paranoid AI coalition reactions, EVA/chat notices and transport are separate
//! from this offline player-list and command implementation.
use crate::map::houses::is_allied_with;
use crate::rules::{overlay_types::OverlayTypeRegistry, ruleset::RuleSet};
use crate::sim::{
    intern::InternedId,
    world::{FrameEffects, Simulation},
};

/// Launch-mode AlliesAllowed and ScenarioFlags0x400. The existing GameOptions
/// remains the sole owner of MultiplayerDialogSettings.AllyChangeAllowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DiplomacyPolicy {
    pub allies_allowed: bool,
    pub fixed_alliance: bool,
}
impl Default for DiplomacyPolicy {
    fn default() -> Self {
        Self {
            allies_allowed: true,
            fixed_alliance: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllianceChangeBlock {
    Unavailable,
    SelfTarget,
    Defeated,
    Computer,
    Locked,
    LastEnemy,
}
impl AllianceChangeBlock {
    pub fn message(self) -> &'static str {
        match self {
            Self::Unavailable => "Alliance changes are unavailable.",
            Self::SelfTarget => "This is your own house.",
            Self::Defeated => "Defeated players cannot change alliances.",
            Self::Computer => "AI alliances are fixed at match start.",
            Self::Locked => "This match does not allow alliance changes.",
            Self::LastEnemy => "You must keep at least one opponent.",
        }
    }
}
impl Simulation {
    pub fn house_considers_ally(&self, source: InternedId, target: InternedId) -> bool {
        is_allied_with(
            &self.house_alliances,
            self.interner.resolve(source),
            self.interner.resolve(target),
        )
    }

    /// CanAlly501540's live (ScenarioInit==0) admission. The prospective ally's
    /// defeated bit is deliberately not tested by the original.
    pub(crate) fn can_make_player_ally(
        &self,
        source: InternedId,
        target: InternedId,
        rules: &RuleSet,
    ) -> bool {
        let (Some(house), Some(other)) = (self.houses.get(&source), self.houses.get(&target))
        else {
            return false;
        };
        if self.house_considers_ally(source, target) || house.is_defeated {
            return false;
        }
        if self.session.game_mode_nonzero
            && rules
                .side_index("Civilian")
                .is_some_and(|side| side.0 == other.side_index)
        {
            return false;
        }
        let mut alive = 0;
        let mut allied = 0;
        for owner in &self.session.house_order {
            if let Some(house) = self.houses.get(owner)
                && !house.is_defeated
                && !house.multiplay_passive
            {
                alive += 1;
                allied += i32::from(self.house_considers_ally(source, *owner));
            }
        }
        alive != allied + 1
    }

    /// Shared producer/receiver gate. A queued command is rechecked at execution.
    pub fn alliance_change_block(
        &self,
        source: InternedId,
        target: InternedId,
        rules: &RuleSet,
    ) -> Option<AllianceChangeBlock> {
        use AllianceChangeBlock::*;
        let (Some(house), Some(other)) = (self.houses.get(&source), self.houses.get(&target))
        else {
            return Some(Unavailable);
        };
        if source == target {
            return Some(SelfTarget);
        }
        if house.is_defeated {
            return Some(Defeated);
        }
        if !house.is_human || !other.is_human {
            return Some(Computer);
        }
        if !self.session.game_mode_nonzero
            || house.multiplay_passive
            || other.multiplay_passive
            || !self.session.diplomacy.allies_allowed
            || self.session.diplomacy.fixed_alliance
            || !self.session.game_options.ally_change_allowed
        {
            return Some(Locked);
        }
        if !self.house_considers_ally(source, target)
            && !self.can_make_player_ally(source, target, rules)
        {
            return Some(LastEnemy);
        }
        None
    }

    pub(crate) fn toggle_player_alliance(
        &mut self,
        source: InternedId,
        target: InternedId,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> bool {
        if self.alliance_change_block(source, target, rules).is_some() {
            return false;
        }
        let source_name = self.interner.resolve(source).trim().to_ascii_uppercase();
        let target_name = self.interner.resolve(target).trim().to_ascii_uppercase();
        let breaking = self.house_considers_ally(source, target);
        let reciprocal = self.house_considers_ally(target, source);
        let mut graph = self.house_alliances.clone();
        if breaking {
            // MakeEnemy4F9FDD applies anger before removing either ally bit.
            crate::sim::house_strategy::update_anger_nodes(
                &mut self.houses,
                &self.session.house_order,
                &self.house_alliances,
                &self.interner,
                source,
                target,
                1,
            );
            if let Some(allies) = graph.get_mut(&source_name) {
                allies.remove(&target_name);
            }
            // MakeEnemy4FA094 clears the reciprocal bit too. It does not merely
            // withdraw the initiating House's offer.
            if let Some(allies) = graph.get_mut(&target_name) {
                allies.remove(&source_name);
            }
        } else {
            graph.entry(source_name).or_default().insert(target_name);
        }
        self.install_house_alliances(
            crate::sim::house_threat::HouseAllianceAdmission::Admitted(graph),
            rules,
        );
        if breaking {
            // The reciprocal call at4FA0DF names the target itself. The shared
            // native anger owner correctly ignores that unregistered self node
            // but still reselects its enemy after the reciprocal bit is removed.
            if reciprocal {
                crate::sim::house_strategy::update_anger_nodes(
                    &mut self.houses,
                    &self.session.house_order,
                    &self.house_alliances,
                    &self.interner,
                    target,
                    target,
                    1,
                );
            }
        } else {
            let delta = self.houses[&source]
                .grudge_scores
                .get(&target)
                .copied()
                .unwrap_or(0)
                .wrapping_neg();
            crate::sim::house_strategy::update_anger_nodes(
                &mut self.houses,
                &self.session.house_order,
                &self.house_alliances,
                &self.interner,
                source,
                target,
                delta,
            );
            let house = self.houses.get_mut(&source).unwrap();
            if house.enemy_house == Some(target) {
                house.enemy_house = None;
            }
            // Original walks Technos and clears every owned, live TarCom that
            // now names an ally. Use the concrete setters (Boris/Magnetron/etc).
            let actors: Vec<_> = self
                .entities()
                .values()
                .filter_map(|entity| {
                    if entity.owner() != source || entity.dying || entity.lifecycle.in_limbo {
                        return None;
                    }
                    let crate::sim::combat::TargetKind::Entity(id) =
                        entity.attack_target.as_ref()?.target
                    else {
                        return None;
                    };
                    let owner = self.entities().get(id)?.owner();
                    self.house_considers_ally(source, owner)
                        .then_some(entity.stable_id())
                })
                .collect();
            for id in actors {
                let _ = self.assign_target_represented(id, None, Some(rules), registry, effects);
            }
        }
        // Queries in the remainder of this frame must see the new graph.
        // The normal sight refresh rebuilds its ephemeral audience cache and
        // removes obsolete sight admissions while retaining explored terrain.
        self.refresh_diplomacy_sight(rules);
        true
    }
}

#[cfg(test)]
#[path = "diplomacy_tests.rs"]
mod tests;
