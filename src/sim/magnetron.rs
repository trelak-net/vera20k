//! Magnetron's IsLocomotor arm4694CB, Imbue710000 and Release70FEE0.
//! The displaced locomotor is owned by the existing serialized piggyback stack.
use crate::map::entities::EntityCategory;
use crate::rules::{overlay_types::OverlayTypeRegistry, ruleset::RuleSet};
use crate::sim::intern::InternedId;
use crate::sim::movement::{air_movement::current_fly_height, locomotor::MovementLayer};
use crate::sim::{
    combat::{EntityDamageEvent, ReceiverCallFlags},
    components::NavTargetRef,
    projectile::ProjectilePayload,
    world::{FrameEffects, Simulation, UninitContext},
};

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub(crate) struct MagnetronState {
    /// Techno+2AC and +2B0, reciprocal live links.
    pub target: Option<u64>,
    pub source: Option<u64>,
    /// Foot+427, +428 and +42C survive release until landing.
    pub dropped: bool,
    /// Exact deck-crossing result from State5, kept until its impact notice.
    pub bridge_impact: bool,
    /// Foot+6AE: permits the native Jumpjet END gate after release/touchdown.
    pub end_requested: bool,
    pub dropper: Option<u64>,
    pub dropper_house: Option<InternedId>,
}
impl MagnetronState {
    pub fn references(self) -> impl Iterator<Item = u64> {
        [self.target, self.source, self.dropper]
            .into_iter()
            .flatten()
    }
}
fn source_admits(
    source_present: bool,
    target: Option<u64>,
    held: Option<u64>,
    source_held: bool,
    swapped: bool,
) -> bool {
    source_present && target != held && !source_held && !swapped
}
fn receiver_admits(
    foot: bool,
    kind: i32,
    ground: bool,
    protected: bool,
    swapped: bool,
    factory: bool,
    damage: i32,
    weight: f64,
) -> bool {
    foot && matches!(kind, 1 | 2)
        && ground
        && !protected
        && !swapped
        && !factory
        && f64::from(damage) > weight
}
///54CAD2..54CAFC: native x87 truncation and accelerating float crash speed.
pub(crate) fn falling_step(z: i32, crash: u32) -> (i32, u32) {
    use crate::util::native_x87::{NativeF32Bits, X87Chop53};
    let speed = X87Chop53::load_f32(NativeF32Bits::from_bits(crash)).unwrap();
    (
        X87Chop53::ftol_i32_low_masked(X87Chop53::sub(X87Chop53::load_i32(z), speed)),
        X87Chop53::store_f32(X87Chop53::add(speed, X87Chop53::load_i32(1)))
            .unwrap()
            .bits(),
    )
}
impl Simulation {
    pub(crate) fn magnetron_detonation(
        &mut self,
        owner: u64,
        target: Option<u64>,
        payload: ProjectilePayload,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(source) = self.substrate.entities.get(owner) else {
            return;
        };
        if !source_admits(
            true,
            target,
            source.magnetron.target,
            source.magnetron.source.is_some(),
            source.foot_locomotor_swap_active,
        ) {
            return;
        }
        //4695EB releases the previous object BEFORE testing the new receiver.
        self.magnetron_release(owner, false, Some(rules), registry, effects);
        let Some(target) = target.filter(|id| *id != owner) else {
            return;
        };
        let Some(victim) = self.substrate.entities.get(target) else {
            return;
        };
        if !victim.is_object_alive() || victim.lifecycle.in_limbo {
            return;
        }
        let factory = victim.category == EntityCategory::Unit
            && victim
                .radio_contacts
                .slot(0)
                .and_then(|id| self.substrate.entities.get(id))
                .is_some_and(|building| {
                    self.object_type(building.type_ref(), rules)
                        .is_some_and(|ty| ty.weapons_factory)
                        && self.substrate.occupancy.first_building_on_layer(
                            victim.position.rx,
                            victim.position.ry,
                            if victim.on_bridge {
                                MovementLayer::Bridge
                            } else {
                                MovementLayer::Ground
                            },
                        ) == Some(building.stable_id())
                });
        let Some(ty) = self.object_type(victim.type_ref(), rules) else {
            return;
        };
        let kind = match victim.category {
            EntityCategory::Unit => 1,
            EntityCategory::Aircraft => 2,
            EntityCategory::Infantry => 15,
            EntityCategory::Structure => 6,
        };
        let ground =
            crate::sim::world::entity_layer(victim, self.resolved_terrain.as_ref(), Some(rules))
                == crate::sim::world::display_layers::DisplayLayer::GROUND;
        if !receiver_admits(
            kind != 6,
            kind,
            ground,
            crate::sim::superweapon::invulnerability::is_invulnerable(
                victim.invulnerability.as_ref(),
                self.session.binary_frame,
            ),
            victim.foot_locomotor_swap_active,
            factory,
            payload.base_damage,
            ty.weight.to_num::<f64>(),
        ) {
            return;
        }
        let Some(kind) = rules
            .warhead(self.interner.resolve(payload.warhead))
            .and_then(|w| w.locomotor)
        else {
            return;
        };
        let params = ty.jumpjet_params.clone();
        let parasite = victim.parasite_eating_me;
        let frame = self.session.binary_frame;
        if victim.locomotor.is_none() {
            return;
        }
        crate::sim::spawn_manager::kill_all_spawns_with_context(
            self,
            target,
            UninitContext::new(Some(rules), registry).with_effects(effects),
        );
        if let Some(parasite) = parasite
            && self
                .substrate
                .entities
                .get(parasite)
                .and_then(|p| self.object_type(p.type_ref(), rules))
                .is_some_and(|ty| ty.naval)
        {
            self.parasite_exit_unit(parasite, rules, effects);
        }
        self.magnetron_release(target, false, Some(rules), registry, effects);
        let _ = self.locomotor_stop_moving(target, Some(rules), registry, effects);
        self.foot_mark_remove(target, Some(rules), registry, effects);
        let victim = self.substrate.entities.get_mut(target).unwrap();
        victim.clear_live_path_head();
        victim.foot_occupation_enabled = true;
        if !victim
            .locomotor
            .as_mut()
            .unwrap()
            .begin_piggyback(kind, frame)
        {
            return;
        }
        if let Some(runtime) = victim.locomotor.as_mut().unwrap().jumpjet_runtime_mut() {
            runtime.link(&params);
        }
        //71031E assigns NavCom while +6AD is still clear. The Foot setter
        //rejects nonnull destinations once the lift flag rises at710352.
        victim.magnetron.source = Some(owner);
        victim.magnetron.dropped = false;
        victim.selected = false;
        self.substrate
            .entities
            .get_mut(owner)
            .unwrap()
            .magnetron
            .target = Some(target);
        self.leave_team(target, true, Some(rules), effects);
        crate::sim::radio::broadcast_break(self, target, None, effects);
        self.foot_mark_put(target, Some(rules), registry, effects);
        let _ = self.assign_destination_represented(
            target,
            Some(NavTargetRef::object(owner)),
            Some(rules),
            registry,
            effects,
        );
        self.substrate
            .entities
            .get_mut(target)
            .unwrap()
            .foot_locomotor_swap_active = true;
        let _ = self.mission_queue_exact(
            target,
            crate::sim::mission::MissionId::from_known(crate::sim::mission::MissionType::Guard),
            1,
            frame,
            &crate::sim::mission::authority::EntityReadyInputProvider,
        );
    }

    pub(crate) fn magnetron_release(
        &mut self,
        owner: u64,
        clear_target: bool,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(source) = self.substrate.entities.get(owner) else {
            return;
        };
        let Some(target) = source.magnetron.target else {
            return;
        };
        let house = source.owner();
        // Clear forward first to make Stun/SetTarget reentry harmless.
        self.substrate
            .entities
            .get_mut(owner)
            .unwrap()
            .magnetron
            .target = None;
        if let Some(victim) = self.substrate.entities.get_mut(target) {
            victim.magnetron.source = None;
            if current_fly_height(victim, self.resolved_terrain.as_ref()) > 0 {
                victim.crashing = true;
                victim.magnetron.dropped = true;
                victim.magnetron.dropper = Some(owner);
                victim.magnetron.dropper_house = Some(house);
                self.techno_death_stun(
                    target,
                    UninitContext::new(rules, registry).with_effects(effects),
                );
                self.assign_null_destination(target, rules, registry, effects);
            } else {
                self.assign_null_destination(target, rules, registry, effects);
                self.substrate
                    .entities
                    .get_mut(target)
                    .unwrap()
                    .magnetron
                    .end_requested = true;
                self.assign_null_destination(target, rules, registry, effects);
            }
        }
        if clear_target {
            let _ = self.assign_target_represented(owner, None, rules, registry, effects);
        }
    }

    pub(crate) fn magnetron_restore(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        if !self
            .substrate
            .entities
            .get(id)
            .is_some_and(|e| e.foot_locomotor_swap_active)
        {
            return;
        }
        self.foot_mark_remove(id, rules, registry, effects);
        self.aircraft_tracker_remove(id);
        let e = self.substrate.entities.get_mut(id).unwrap();
        if let Some(l) = e.locomotor.as_mut() {
            l.end_piggyback();
        }
        e.foot_locomotor_swap_active = false;
        e.magnetron = Default::default();
        e.crashing = false;
        e.movement_target = None;
        e.navigation.nav_com = None;
        e.navigation.nav_com_aux = None;
        e.clear_live_path_head();
        self.set_object_height_unmarked(id, 0);
        self.foot_mark_put(id, rules, registry, effects);
    }

    pub(crate) fn magnetron_pointer_expired(
        &mut self,
        listener: u64,
        expired: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(state) = self.substrate.entities.get(listener).map(|e| e.magnetron) else {
            return;
        };
        if !state.references().any(|id| id == expired) {
            return;
        }
        if state.source == Some(expired) {
            self.magnetron_release(expired, false, rules, registry, effects);
        }
        if let Some(e) = self.substrate.entities.get_mut(listener) {
            if e.magnetron.target == Some(expired) {
                e.magnetron.target = None;
            }
            if e.magnetron.source == Some(expired) {
                e.magnetron.source = None;
            }
            if e.magnetron.dropper == Some(expired) {
                e.magnetron.dropper = None;
            }
        }
    }

    /// State5's ground collision54CC47. Uses the existing ordered cell list,
    /// synchronous damage receiver and sinking owner; no world scan.
    pub(crate) fn magnetron_impact(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(e) = self.substrate.entities.get(id) else {
            return;
        };
        let state = e.magnetron;
        let (rx, ry) = (e.position.rx, e.position.ry);
        let source = state.dropper.unwrap_or(id);
        let cell = self.resolved_terrain.as_ref().and_then(|t| t.cell(rx, ry));
        let bridge = state.bridge_impact;
        let water = cell.is_some_and(|c| c.is_water) && !bridge;
        let terrain_passable = self.object_type(e.type_ref(), rules).is_some_and(|ty| {
            cell.is_some_and(|c| {
                bridge || c.speed_costs.cost_for_speed_type(ty.speed_type).is_some()
            })
        });
        let layer = if bridge {
            MovementLayer::Bridge
        } else {
            MovementLayer::Ground
        };
        let below = self
            .substrate
            .occupancy
            .get(rx, ry)
            .map(|c| {
                c.iter_layer(layer)
                    .map(|other| other.entity_id)
                    .filter(|other| *other != id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut die = false;
        if let Some(first) = below
            .first()
            .and_then(|id| self.substrate.entities.get(*id))
        {
            match first.category {
                EntityCategory::Unit | EntityCategory::Structure => die = true,
                EntityCategory::Infantry => {
                    //54CCDE: infantry are recorded and UnInited synchronously,
                    //not left blocking the landing cell in a dying animation.
                    for other in below {
                        if let Some(victim) = self.substrate.entities.get_mut(other) {
                            victim.health.current = 0;
                        }
                        self.record_the_kill(
                            other,
                            Some(source),
                            state.dropper_house,
                            crate::sim::combat::KillCallback::Terminal,
                            rules,
                        );
                        self.uninit_with_context(
                            other,
                            UninitContext::new(Some(rules), registry).with_effects(effects),
                        );
                    }
                }
                EntityCategory::Aircraft => self.magnetron_crush_damage(
                    first.stable_id(),
                    source,
                    state.dropper_house,
                    rules,
                    registry,
                    effects,
                ),
            }
        }
        if let Some(e) = self.substrate.entities.get_mut(id) {
            e.on_bridge = bridge;
        }
        let passable = terrain_passable
            && self
                .mover_can_enter(
                    id,
                    (rx as i16, ry as i16),
                    crate::sim::movement::infantry_entry::InfantryEntryArgs::REPAIR,
                    crate::sim::movement::infantry_entry::EntryQueryMode::CheckLocomotor,
                    rules,
                    registry,
                )
                .is_ok_and(|answer| answer == 0);
        if !die && passable {
            //54CF9F: surviving drops re-enter State4 and finish through the
            //ordinary touchdown/per-cell/END owners on the following visit.
            self.set_object_height(id, 0, Some(rules), registry, effects);
            let e = self.substrate.entities.get_mut(id).unwrap();
            let coordinate = crate::sim::movement::ground_pose::position_world_coord(&e.position);
            e.locomotor
                .as_mut()
                .unwrap()
                .jumpjet_runtime_mut()
                .unwrap()
                .land_magnetron_drop(coordinate);
            e.movement_target = None;
            e.navigation.nav_com = None;
            return;
        }
        self.magnetron_restore(id, Some(rules), registry, effects);
        if die || (!passable && !water) {
            self.magnetron_crush_damage(id, source, state.dropper_house, rules, registry, effects);
        } else if !passable && water {
            self.record_the_kill(
                id,
                Some(source),
                state.dropper_house,
                crate::sim::combat::KillCallback::Terminal,
                rules,
            );
            self.begin_warp_sinking(id, rules, effects);
        }
    }
    fn magnetron_crush_damage(
        &mut self,
        id: u64,
        source: u64,
        house: Option<InternedId>,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(health) = self
            .substrate
            .entities
            .get(id)
            .map(|e| e.health.current.max(1))
        else {
            return;
        };
        let wh = self.interner.intern(&rules.bridge_warheads.c4_name);
        let event = EntityDamageEvent::direct_receiver(
            id,
            health,
            0,
            source,
            house,
            wh,
            ReceiverCallFlags {
                ignore_defenses: true,
                arg6: false,
            },
        );
        self.commit_direct_damage_receiver(rules, registry, event, effects);
    }
}

#[cfg(test)]
#[path = "magnetron_tests.rs"]
mod tests;
