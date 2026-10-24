//! Aegis fire distribution: candidate vectors at Techno+440/+458,
//! firing history at +470; selection709550, FireAt tail6FF872 and expiry707BCE.
use crate::rules::{overlay_types::OverlayTypeRegistry, ruleset::RuleSet};
use crate::sim::{
    combat::TargetKind,
    world::{FrameEffects, Simulation},
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct DistributedFire {
    candidates: Vec<(u64, i32)>,
    history: Vec<TargetKind>,
}
impl DistributedFire {
    pub(crate) fn replace_candidates(&mut self, candidates: Vec<(u64, i32)>) {
        self.candidates = candidates;
    }
    pub(crate) fn record(&mut self, target: TargetKind) {
        self.history.push(target);
    }
    pub(crate) fn references(&self) -> impl Iterator<Item = u64> + '_ {
        self.candidates
            .iter()
            .map(|entry| entry.0)
            .chain(self.history.iter().filter_map(|target| {
                if let TargetKind::Entity(id) = target {
                    Some(*id)
                } else {
                    None
                }
            }))
    }
    pub(crate) fn names(&self, id: u64) -> bool {
        self.references().any(|entry| entry == id)
    }
    pub(crate) fn forget(&mut self, id: u64) {
        // Clear every matching stable handle, including repeated cell-scan
        // candidates, so destroyed objects cannot leave dangling save links.
        self.candidates.retain(|entry| entry.0 != id);
        self.history
            .retain(|target| *target != TargetKind::Entity(id));
    }
    /// Outer None is the native empty-list early return, without AssignTarget.
    fn choose(&mut self) -> Option<Option<u64>> {
        if self.candidates.is_empty() {
            self.history.clear();
            return None;
        }
        self.history.retain(|target| {
            self.candidates
                .iter()
                .any(|entry| *target == TargetKind::Entity(entry.0))
        });
        let has_unvisited = self
            .candidates
            .iter()
            .any(|entry| !self.history.contains(&TargetKind::Entity(entry.0)));
        if !has_unvisited {
            self.history.clear();
        }
        let mut best = None;
        let mut score = 0;
        for &(id, value) in &self.candidates {
            if !self.history.contains(&TargetKind::Entity(id)) && value > score {
                best = Some(id);
                score = value;
            }
        }
        Some(best)
    }
}
impl Simulation {
    pub(crate) fn distribute_fire(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(entity) = self.substrate.entities.get_mut(id) else {
            return;
        };
        let assignment = entity.distributed_fire.choose();
        if let Some(target) = assignment {
            let _ = self.assign_target_represented(
                id,
                target.map(TargetKind::Entity),
                Some(rules),
                registry,
                effects,
            );
        }
    }
    pub(crate) fn distributed_fire_tail(
        &mut self,
        id: u64,
        target: TargetKind,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(entity) = self.entities().get(id) else {
            return;
        };
        if !self
            .object_type(entity.type_ref(), rules)
            .is_some_and(|kind| kind.distributed_fire)
            || (entity.category != crate::map::entities::EntityCategory::Structure
                && entity.mission.current().known()
                    == Some(crate::sim::mission::MissionType::Attack))
        {
            return;
        }
        self.substrate
            .entities
            .get_mut(id)
            .unwrap()
            .distributed_fire
            .record(target);
        let _ = self.assign_target_represented(id, None, Some(rules), registry, effects);
    }
    pub(crate) fn distributed_fire_pointer_expired(&mut self, listener: u64, expired: u64) {
        if self
            .entities()
            .get(listener)
            .is_some_and(|e| e.distributed_fire.names(expired))
        {
            self.substrate
                .entities
                .get_mut(listener)
                .unwrap()
                .distributed_fire
                .forget(expired);
        }
    }
}
#[cfg(test)]
#[path = "distributed_fire_tests.rs"]
mod tests;
