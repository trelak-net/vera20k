//! Boris AirstrikeClass: flare469705, admission41D7E0, launch41D860,
//! cancellation41DB40 and member removal41DC80. Reuses ordinary aircraft
//! construction, edge search, attack, retreat, damage and deletion owners.
use crate::map::entities::EntityCategory;
use crate::rules::{overlay_types::OverlayTypeRegistry, ruleset::RuleSet};
use crate::sim::{
    combat::TargetKind,
    mission::{MissionId, MissionType},
    rng::SimRng,
    timer::CdTimer,
    world::{FrameEffects, PlacementEvidence, Simulation},
};
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct AirstrikeState {
    pub manager: Option<AirstrikeManager>,
    /// Native Techno+294 on a member aircraft or the designated building.
    pub controller: Option<u64>,
    pub tint: AirstrikeTint,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct AirstrikeManager {
    pub target: Option<u64>,
    /// Native NextTeamMember chain, head first. No ordinary-frame census.
    pub planes: Vec<u64>,
    pub active: bool,
    pub last_finished: i32,
}
impl AirstrikeState {
    pub fn references(&self) -> impl Iterator<Item = u64> + '_ {
        self.controller.into_iter().chain(
            self.manager
                .iter()
                .flat_map(|m| m.target.into_iter().chain(m.planes.iter().copied())),
        )
    }
}
impl AirstrikeManager {
    fn can_fire(&self, target: u64, recharge: i32, frame: i32) -> bool {
        (!self.active && self.last_finished.wrapping_add(recharge) <= frame)
            || self.target != Some(target)
    }
}
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub(crate) struct AirstrikeTint {
    pub timer: CdTimer,
    pub stage_timer: CdTimer,
    pub stage: u8,
}
impl AirstrikeTint {
    fn start(&mut self, frame: i32) {
        self.timer.start(frame, 100000);
        self.stage = 0;
    }
    fn update(&mut self, frame: i32, rng: &mut SimRng) {
        let left = self.timer.remaining(frame);
        let expired = self.stage_timer.expired(frame);
        let next = match self.stage {
            0 => Some((1, 6)),
            1 => expired.then_some((2, 4)),
            2 => expired.then(|| (3, 20 + rng.next_range_i32_inclusive(-5, 5))),
            3 => expired.then_some((4, 64)),
            4 => expired.then_some((5, 64)),
            5 if expired && left < 158 => {
                self.stage = 6;
                None
            }
            5 => expired.then_some((4, 64)),
            6 => (left <= 30).then_some((7, 6)),
            7 => expired.then_some((8, 4)),
            8 => expired.then_some((9, 20)),
            9 if !expired => None,
            _ => {
                self.stage = 10;
                None
            }
        };
        if let Some((stage, duration)) = next {
            self.stage = stage;
            self.stage_timer.start(frame, duration);
        }
    }
    pub(crate) fn intensity(&self, value: i32, frame: i32) -> i32 {
        let left = self.stage_timer.remaining(frame);
        let scale = match self.stage {
            1 | 7 => (12i32.wrapping_sub(left) << 8) / 6,
            2 | 8 => 512,
            3 | 9 => (left.wrapping_add(20) << 8) / 20,
            4 => (128i32.wrapping_sub(left) << 8) / 64,
            5 => (left.wrapping_add(64) << 8) / 64,
            6 => 256,
            _ => return value,
        };
        (scale.wrapping_mul(value) >> 8).min(2000)
    }
}
impl Simulation {
    pub(crate) fn airstrike_detonation(
        &mut self,
        owner: u64,
        target: Option<u64>,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(target) = target.filter(|id| self.substrate.entities.contains(*id)) else {
            return;
        };
        let Some(source) = self.substrate.entities.get(owner) else {
            return;
        };
        let Some(manager) = source.airstrike.manager.as_ref() else {
            return;
        };
        let Some(ty) = self.object_type(source.type_ref(), rules) else {
            return;
        };
        let elite = source.veterancy() >= 200;
        let recharge = if elite {
            ty.elite_airstrike_recharge_time
        } else {
            ty.airstrike_recharge_time
        };
        if !manager.can_fire(target, recharge, self.session.binary_frame as i32) {
            return;
        }
        if manager.active {
            let planes = manager.planes.clone();
            let first_ammo = planes
                .first()
                .and_then(|id| self.substrate.entities.get(*id))
                .and_then(|e| e.aircraft_ammo.as_ref())
                .map(|a| a.current);
            if planes.iter().any(|id| {
                self.substrate
                    .entities
                    .get(*id)
                    .and_then(|e| e.aircraft_ammo.as_ref())
                    .map(|a| a.current)
                    != first_ammo
            }) {
                self.airstrike_cancel(owner, Some(rules), registry, effects);
                return;
            }
            self.airstrike_clear_mark(owner);
            self.substrate
                .entities
                .get_mut(owner)
                .unwrap()
                .airstrike
                .manager
                .as_mut()
                .unwrap()
                .target = Some(target);
            self.airstrike_mark(owner, target);
            for plane in planes {
                if self
                    .substrate
                    .entities
                    .get(plane)
                    .and_then(|e| e.aircraft_ammo.as_ref())
                    .is_some_and(|a| a.current > 0)
                {
                    let _ = self.assign_target_represented(
                        plane,
                        Some(TargetKind::Entity(target)),
                        Some(rules),
                        registry,
                        effects,
                    );
                    let _ = self.mission_queue_exact(
                        plane,
                        MissionId::from_known(MissionType::Attack),
                        0,
                        self.session.binary_frame,
                        &crate::sim::mission::authority::EntityReadyInputProvider,
                    );
                }
            }
            return;
        }
        let count = if elite {
            ty.elite_airstrike_team
        } else {
            ty.airstrike_team
        };
        let plane_type = if elite {
            ty.elite_airstrike_team_type.clone()
        } else {
            ty.airstrike_team_type.clone()
        };
        let Some(plane_type) = plane_type.filter(|name| {
            rules
                .aircraft_ids
                .iter()
                .any(|id| id.eq_ignore_ascii_case(name))
        }) else {
            return;
        };
        let house = source.owner();
        let owner_name = self.interner.resolve(house).to_string();
        let point = crate::sim::movement::ground_pose::object_get_coords(
            self.substrate.entities.get(target).unwrap(),
            self.resolved_terrain.as_ref(),
        );
        let target_cell = ((point.x / 256) as u16, (point.y / 256) as u16);
        let (authored, waypoint) = self
            .houses
            .get(&house)
            .map_or((-1, 0), |h| (h.authored_edge(), h.waypoint_edge));
        //65E881: one edge pick BEFORE any constructors, shared by the formation.
        let edge = crate::sim::world::edge_cell::Edge::authored_or_waypoint(authored, waypoint);
        let Some(edge_cell) = crate::sim::world::edge_cell::find_paradrop_edge_cell(
            self.playfield_bounds,
            self.resolved_terrain.as_ref(),
            edge,
            &mut self.scenario_rng,
        ) else {
            return;
        };
        self.substrate
            .entities
            .get_mut(owner)
            .unwrap()
            .airstrike
            .manager
            .as_mut()
            .unwrap()
            .active = true;
        for _ in 0..count.max(0) {
            let Some(plane) = self.with_object_placement_scope(|sim| {
                sim.construct_object_limbo_at_height(&plane_type, &owner_name, 0, 0, 0, 0, rules)
            }) else {
                break;
            };
            let e = self.substrate.entities.get_mut(plane).unwrap();
            e.airstrike.controller = Some(owner);
            e.mission_leaf.set_airstrike_manager_present(true);
            e.mark_mission_only();
            self.substrate
                .entities
                .get_mut(owner)
                .unwrap()
                .airstrike
                .manager
                .as_mut()
                .unwrap()
                .planes
                .insert(0, plane);
            let cell = self
                .find_plain_passable_cell(
                    (i32::from(edge_cell.0), i32::from(edge_cell.1)),
                    crate::rules::locomotor_type::SpeedType::Winged,
                    None,
                    crate::rules::locomotor_type::MovementZone::Normal,
                    (1, 1),
                )
                .unwrap_or(edge_cell);
            let e = self.substrate.entities.get_mut(plane).unwrap();
            crate::sim::mission::authority::queue_entity_mission_deferred(
                e,
                MissionId::from_known(MissionType::Attack),
            );
            crate::sim::mission::concrete_effects::represented_assign_target(
                e,
                Some(TargetKind::Cell(target_cell.0, target_cell.1)),
            );
            let placed = self.with_object_placement_scope(|sim| {
                sim.reveal_constructed_object_at_height(
                    plane,
                    cell.0,
                    cell.1,
                    0,
                    0,
                    PlacementEvidence::MarkSucceeded,
                    rules,
                    effects,
                )
            });
            if placed.is_none() {
                self.discard_constructed_limbo(plane, Some(rules), effects);
                break;
            }
            crate::sim::mission::authority::commence_entity_mission(
                self.substrate.entities.get_mut(plane).unwrap(),
                self.session.binary_frame,
            );
        }
        let planes = &self
            .substrate
            .entities
            .get(owner)
            .unwrap()
            .airstrike
            .manager
            .as_ref()
            .unwrap()
            .planes;
        if planes.is_empty() {
            let m = self
                .substrate
                .entities
                .get_mut(owner)
                .unwrap()
                .airstrike
                .manager
                .as_mut()
                .unwrap();
            m.active = false;
            m.last_finished = self.session.binary_frame as i32;
            return;
        }
        let first = planes[0];
        self.airstrike_report(first, rules.general.airstrike_attack_voice.as_deref());
        self.substrate
            .entities
            .get_mut(owner)
            .unwrap()
            .airstrike
            .manager
            .as_mut()
            .unwrap()
            .target = Some(target);
        self.airstrike_mark(owner, target);
    }
    fn airstrike_report(&mut self, plane: u64, sound: Option<&str>) {
        if let Some(sound) = sound
            && let Some(e) = self.substrate.entities.get(plane)
        {
            let p = crate::sim::movement::ground_pose::position_world_coord(&e.position);
            let world = crate::sim::anim_class::AnimWorldCoord {
                x: p.x,
                y: p.y,
                z: p.z,
            };
            let sound_id = self.interner.intern(sound);
            self.sound_events
                .push(crate::sim::world::SimSoundEvent::ObjectSoundStarted {
                    owner: plane,
                    sound_id,
                    world,
                });
        }
    }
    fn airstrike_mark(&mut self, owner: u64, target: u64) {
        if let Some(e) = self.substrate.entities.get_mut(target)
            && e.category == EntityCategory::Structure
        {
            e.airstrike.controller = Some(owner);
            e.airstrike.tint.start(self.session.binary_frame as i32);
        }
    }
    fn airstrike_clear_mark(&mut self, owner: u64) {
        let Some(target) = self
            .substrate
            .entities
            .get(owner)
            .and_then(|e| e.airstrike.manager.as_ref())
            .and_then(|m| m.target)
        else {
            return;
        };
        if !self
            .substrate
            .entities
            .get(target)
            .is_some_and(|e| e.airstrike.controller == Some(owner))
        {
            return;
        }
        //41DBE8: last other manager still naming this building takes the mark.
        let other = self
            .substrate
            .entities
            .values_sorted()
            .filter(|e| {
                e.stable_id() != owner
                    && e.airstrike
                        .manager
                        .as_ref()
                        .is_some_and(|m| m.target == Some(target))
            })
            .map(|e| e.stable_id())
            .last();
        if let Some(e) = self.substrate.entities.get_mut(target) {
            e.airstrike.controller = other;
        }
    }
    pub(crate) fn airstrike_cancel(
        &mut self,
        owner: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(manager) = self
            .substrate
            .entities
            .get(owner)
            .and_then(|e| e.airstrike.manager.as_ref())
        else {
            return;
        };
        if manager.target.is_none() && manager.planes.is_empty() {
            return;
        }
        let planes = manager.planes.clone();
        let spent = planes.iter().any(|id| {
            self.substrate
                .entities
                .get(*id)
                .and_then(|e| e.aircraft_ammo.as_ref())
                .is_some_and(|a| a.current == 0)
        });
        self.airstrike_clear_mark(owner);
        self.substrate
            .entities
            .get_mut(owner)
            .unwrap()
            .airstrike
            .manager
            .as_mut()
            .unwrap()
            .target = None;
        for plane in planes {
            if let Some(e) = self.substrate.entities.get_mut(plane) {
                if spent && let Some(ammo) = e.aircraft_ammo.as_mut() {
                    ammo.current = 0;
                }
                crate::sim::mission::authority::queue_entity_mission_deferred(
                    e,
                    MissionId::from_known(MissionType::Retreat),
                );
            }
            let _ = self.assign_target_represented(plane, None, rules, registry, effects);
            self.assign_null_destination(plane, rules, registry, effects);
        }
    }
    pub(crate) fn airstrike_pointer_expired(
        &mut self,
        listener: u64,
        expired: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(state) = self.substrate.entities.get(listener).map(|e| &e.airstrike) else {
            return;
        };
        if !state.references().any(|id| id == expired) {
            return;
        }
        let target_expired = state
            .manager
            .as_ref()
            .is_some_and(|m| m.target == Some(expired));
        let controller_expired = state.controller == Some(expired);
        if target_expired {
            self.airstrike_cancel(listener, rules, registry, effects);
        }
        //41D58B: destroying the manager owner aborts its whole flight even
        //when UnInit was reached without the damage/Stun path (map actions).
        if controller_expired
            && self
                .substrate
                .entities
                .get(expired)
                .and_then(|e| e.airstrike.manager.as_ref())
                .is_some_and(|m| m.target.is_some())
        {
            if let Some(rules) = rules {
                let source = self.substrate.entities.get(expired).unwrap();
                let planes = &source.airstrike.manager.as_ref().unwrap().planes;
                let unfinished = planes.iter().any(|id| {
                    self.substrate
                        .entities
                        .get(*id)
                        .and_then(|e| e.aircraft_ammo.as_ref())
                        .is_some_and(|a| a.current != 0)
                });
                if unfinished
                    && self.owner_is_human(source.owner())
                    && let Some(plane) = planes.first().copied()
                {
                    self.airstrike_report(plane, rules.general.airstrike_abort_sound.as_deref());
                }
            }
            self.airstrike_cancel(expired, rules, registry, effects);
        }
        let e = self.substrate.entities.get_mut(listener).unwrap();
        if let Some(m) = e.airstrike.manager.as_mut() {
            let len = m.planes.len();
            m.planes.retain(|id| *id != expired);
            if len != m.planes.len() && m.planes.is_empty() {
                m.active = false;
                m.last_finished = self.session.binary_frame as i32;
            }
        }
        if e.airstrike.controller == Some(expired) {
            e.airstrike.controller = None;
            if e.category == EntityCategory::Aircraft {
                e.mission_leaf.set_airstrike_manager_present(false);
            }
        }
    }
    pub(crate) fn airstrike_tint_step(&mut self, id: u64) {
        let Some(e) = self.substrate.entities.get(id) else {
            return;
        };
        let active = e.category == EntityCategory::Structure
            && e.airstrike
                .controller
                .and_then(|id| self.substrate.entities.get(id))
                .and_then(|e| e.airstrike.manager.as_ref())
                .is_some_and(|m| m.target == Some(id));
        if !active && e.airstrike.tint.stage == 0 {
            return;
        }
        let tint = &mut self.substrate.entities.get_mut(id).unwrap().airstrike.tint;
        if active {
            tint.update(self.session.binary_frame as i32, &mut self.scenario_rng)
        } else {
            tint.stage = 0;
        }
    }
}
#[cfg(test)]
#[path = "airstrike_tests.rs"]
mod tests;
