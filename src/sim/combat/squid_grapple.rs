//! Parasite grapple6297F0 and stage629720. Original executed timelines live in
//! tools/spatial_oracle/squid_grapple.json. Damage/expiry remain synchronous:
//! do not draw the next direction after a receiver detached the victim.
use super::*;
use crate::map::retail_trig::TrigTable;
use crate::sim::anim_class::AnimWorldCoord;
use crate::sim::components::{AnimClassSpawnDescriptor, RockingState};
use crate::util::native_x87::{NativeF32Bits, NativeF64Bits, X87Chop53 as Fpu};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(super) struct GrappleState {
    pub(super) anim: Option<u64>,
    phase: i32,
    stage: i32,
    accumulator: i32,
    roll: u32,
}
impl GrappleState {
    /// 629720: the tenth stage returns before writing the animation frame.
    fn step(&mut self, facing: u16) -> (bool, Option<i32>) {
        self.accumulator += 1;
        let threshold = if self.phase <= 1 { 3 } else { 4 };
        if self.accumulator < threshold {
            return (false, None);
        }
        self.accumulator = 0;
        self.stage += 1;
        if self.stage >= 10 {
            return (true, None);
        }
        let base = match self.phase {
            0 | 1 => 0,
            2 => 80,
            _ => 160,
        };
        (false, Some(base + 10 * dir8(facing) as i32 + self.stage))
    }
    fn enter(&mut self, phase: i32, facing: u16) {
        self.phase = phase;
        self.stage = 0;
        self.accumulator = 0;
        self.step(facing);
    }
    fn rock(&mut self, negative: bool) {
        let signed = if negative { -self.stage } else { self.stage };
        let angle = Fpu::mul(
            Fpu::load_i32(signed),
            Fpu::load_f64(NativeF64Bits::from_bits(0x3fd41b2f769cf0e0)).unwrap(),
        );
        let angle = Fpu::load_f64(Fpu::store_f64(angle).unwrap()).unwrap();
        let roll = Fpu::mul(
            TrigTable::embedded().sin_from_table(angle),
            Fpu::load_f32(NativeF32Bits::from_bits(0x3f490fdb)).unwrap(),
        );
        self.roll = Fpu::store_f32(roll).unwrap().bits();
    }
    fn restart_rock(&mut self, facing: u16, draw: i32) {
        self.enter(if draw == 0 { 3 } else { 2 }, facing);
    }
}
/// 629EB9 / 629A85: signed facing, negated angle; truncate displacement
/// BEFORE adding the origin. Splash longitudinal offsets are -128,0,128.
fn displaced(origin: DriveCoord, facing: u16, x: i32, y: i32) -> DriveCoord {
    let angle = Fpu::neg(Fpu::mul(
        Fpu::load_i32(i32::from(facing as i16) - 0x3fff),
        Fpu::load_f64(NativeF64Bits::from_bits(0xbf19222d989f5e57)).unwrap(),
    ));
    let angle = Fpu::load_f64(Fpu::store_f64(angle).unwrap()).unwrap();
    let sin = TrigTable::embedded().sin_from_table(angle);
    let cos = TrigTable::embedded().cos_from_table(angle);
    let (x, y) = (Fpu::load_i32(x), Fpu::load_i32(y));
    DriveCoord {
        x: origin.x.wrapping_add(Fpu::ftol_i32_low_masked(Fpu::sub(
            Fpu::mul(x, cos),
            Fpu::mul(y, sin),
        ))),
        y: origin.y.wrapping_add(Fpu::ftol_i32_low_masked(Fpu::add(
            Fpu::mul(x, sin),
            Fpu::mul(y, cos),
        ))),
        z: origin.z,
    }
}
/// HealthStatus5F5DD0 and IsElite750010 at629BB2..629BF3.
fn should_cull(
    enabled: bool,
    health: i32,
    strength: i32,
    elite: bool,
    red: f64,
    yellow: f64,
) -> bool {
    if !enabled || health <= 0 {
        return false;
    }
    let fraction = Fpu::div(Fpu::load_i32(health), Fpu::load_i32(strength.max(1))).unwrap();
    let threshold = if elite { yellow } else { red };
    Fpu::compare(
        fraction,
        Fpu::load_f64(NativeF64Bits::from_bits(threshold.to_bits())).unwrap(),
    ) != crate::util::native_x87::X87Ordering::Greater
}

impl Simulation {
    /// Techno WhatAction700354..7003D6: a Sonic weapon may attack a
    /// friendly Foot while a parasite occupies it. Shared by click, cursor
    /// and semantic command admission, with an isolated map query for UI.
    pub(crate) fn can_sonic_rescue(&self, firer: u64, victim: u64, rules: &RuleSet) -> bool {
        let (Some(actor), Some(target)) = (self.entities().get(firer), self.entities().get(victim))
        else {
            return false;
        };
        if actor.stable_id() == target.stable_id()
            || target.parasite_eating_me.is_none()
            || !crate::map::houses::is_allied_with(
                &self.house_alliances,
                self.interner.resolve(actor.owner()),
                self.interner.resolve(target.owner()),
            )
        {
            return false;
        }
        let Some(object) = self.object_type(actor.type_ref(), rules) else {
            return false;
        };
        let cells = self
            .resolved_terrain
            .as_ref()
            .map(crate::map::resolved_terrain::NativeCellQuery::isolated);
        super::super::combat_weapon::select_weapon_against_in_query(
            rules,
            actor,
            object,
            &super::super::combat_weapon::attacker_facts(actor, object),
            Some(&crate::sim::combat::TargetKind::Entity(victim)),
            self.entities(),
            &self.interner,
            cells.as_ref(),
            Some(&self.house_alliances),
        )
        .and_then(|(_, weapon)| weapon)
        .and_then(|w| w.weapon.warhead.as_deref())
        .and_then(|name| rules.warhead(name))
        .is_some_and(|w| w.sonic)
    }

    pub(crate) fn clear_squid_grapple(&mut self, owner: u64, rules: Option<&RuleSet>) {
        let Some(state) = self
            .substrate
            .entities
            .get_mut(owner)
            .and_then(|e| e.parasite.as_deref_mut())
        else {
            return;
        };
        let victim = state.victim;
        let old = std::mem::take(&mut state.grapple);
        if old.phase != 0 {
            if let Some(victim) = victim.and_then(|v| self.substrate.entities.get_mut(v)) {
                victim.rocking = None;
                victim.paralysis_timer = CdTimer::started(self.session.binary_frame as i32, 0);
            }
        }
        if let Some(anim) = old.anim {
            self.destroy_anim_with_context(anim, rules);
        }
    }
    /// Anim expiry routes through its victim back-link, with no world census.
    pub(crate) fn squid_anim_expired(&mut self, anim: u64, victim: Option<u64>) {
        let owner = victim
            .and_then(|v| self.substrate.entities.get(v))
            .and_then(|v| v.parasite_eating_me);
        if let Some(state) = owner
            .and_then(|o| self.substrate.entities.get_mut(o))
            .and_then(|o| o.parasite.as_deref_mut())
        {
            if state.grapple.anim == Some(anim) {
                state.grapple.anim = None;
            }
        }
    }
    fn squid_anim(
        &mut self,
        name: &str,
        coord: DriveCoord,
        delay: u16,
        z_adjust: i32,
        rules: &RuleSet,
    ) -> Option<u64> {
        let world = AnimWorldCoord {
            x: coord.x,
            y: coord.y,
            z: coord.z,
        };
        let (rx, ry, sx, sy, z) = world.to_cell_sub_z();
        let descriptor = AnimClassSpawnDescriptor {
            delay,
            z_adjust,
            loop_count: 1,
            draw_flags: 0x600,
            ..AnimClassSpawnDescriptor::new(self.interner.intern(name), rx, ry, sx, sy, z)
        };
        self.spawn_anim_at_world(rules, descriptor, world).ok()
    }
    fn squid_wakes(&mut self, coord: DriveCoord, facing: u16, rules: &RuleSet) {
        for _ in 0..3 {
            let x = self.scenario_rng.next_range_i32_inclusive(-180, 180);
            let y = self.scenario_rng.next_range_i32_inclusive(-64, 64);
            self.squid_anim(
                &rules.general.wake.name,
                displaced(coord, facing, x, y),
                0,
                0,
                rules,
            );
        }
    }
    fn squid_splashes(&mut self, coord: DriveCoord, facing: u16, rules: &RuleSet) {
        let splashes = &rules.combat_damage.splash_list;
        for i in 0..3 {
            let y = if self.scenario_rng.next_range_i32_inclusive(0, 1) == 0 {
                64
            } else {
                -64
            };
            // Empty mod lists have no valid native animation index; preserve side draws.
            if !splashes.is_empty() {
                let index = self
                    .scenario_rng
                    .next_range_i32_inclusive(0, splashes.len() as i32 - 1);
                self.squid_anim(
                    &splashes[index as usize],
                    displaced(coord, facing, i * 128 - 128, y),
                    2,
                    -10,
                    rules,
                );
            }
        }
    }
    fn squid_restart_rock(&mut self, owner: u64, coord: DriveCoord, facing: u16, rules: &RuleSet) {
        let draw = self.scenario_rng.next_range_i32_inclusive(0, 1);
        self.substrate
            .entities
            .get_mut(owner)
            .unwrap()
            .parasite
            .as_deref_mut()
            .unwrap()
            .grapple
            .restart_rock(facing, draw);
        self.squid_wakes(coord, facing, rules);
    }
    pub(super) fn squid_grapple_ai(
        &mut self,
        owner: u64,
        victim: u64,
        rules: &RuleSet,
        registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let frame = self.session.binary_frame;
        let Some(eater) = self.substrate.entities.get(owner) else {
            return;
        };
        let Some(object) = self.object_type(eater.type_ref(), rules) else {
            return;
        };
        let Some(weapon) = weapon_zero(rules, object, eater.veterancy()) else {
            return;
        };
        let Some(wh_name) = weapon.warhead.as_deref() else {
            return;
        };
        let Some(wh) = rules.warhead(wh_name) else {
            return;
        };
        let (damage, paralyzes, culling, elite, owner_house) = (
            weapon.damage,
            wh.paralyzes,
            wh.culling,
            eater.veterancy() >= crate::sim::combat::veterancy::RANK_ELITE_U16,
            eater.owner(),
        );
        let Some(ship) = self.substrate.entities.get(victim) else {
            return;
        };
        let on_water = self.resolved_terrain.as_ref().is_none_or(|grid| {
            grid.native_cell_is_water_set_tile(
                grid.native_cell_identity((ship.position.rx as i16, ship.position.ry as i16)),
            )
        });
        if !on_water {
            self.parasite_exit_unit(owner, rules, effects);
            if let Some(eater) = self.substrate.entities.get_mut(owner) {
                eater.paralysis_timer = CdTimer::started(frame as i32, 0);
            }
            return;
        }
        let coord = ground_pose::position_world_coord(&ship.position);
        let facing = ship.body_facing_current(frame);
        let phase = self
            .substrate
            .entities
            .get(owner)
            .unwrap()
            .parasite
            .as_ref()
            .unwrap()
            .grapple
            .phase;
        self.substrate
            .entities
            .get_mut(victim)
            .unwrap()
            .paralysis_timer = CdTimer::started(frame as i32, paralyzes);
        let mut anim_frame = None;
        match phase {
            0 => {
                let anim = self.squid_anim("SQDG", coord, 0, 0, rules);
                let state = &mut self
                    .substrate
                    .entities
                    .get_mut(owner)
                    .unwrap()
                    .parasite
                    .as_deref_mut()
                    .unwrap()
                    .grapple;
                state.anim = anim;
                state.roll = 0;
                state.enter(1, facing);
            }
            1 => {
                let (done, stage) = self
                    .substrate
                    .entities
                    .get_mut(owner)
                    .unwrap()
                    .parasite
                    .as_deref_mut()
                    .unwrap()
                    .grapple
                    .step(facing);
                anim_frame = stage;
                if done {
                    self.squid_restart_rock(owner, coord, facing, rules);
                }
            }
            2 | 3 => {
                let state = &mut self
                    .substrate
                    .entities
                    .get_mut(owner)
                    .unwrap()
                    .parasite
                    .as_deref_mut()
                    .unwrap()
                    .grapple;
                let (done, stage) = state.step(facing);
                anim_frame = stage;
                if done {
                    state.enter(4, facing);
                }
                state.rock(phase == 2);
            }
            4 => {
                self.substrate
                    .entities
                    .get_mut(owner)
                    .unwrap()
                    .parasite
                    .as_deref_mut()
                    .unwrap()
                    .grapple
                    .roll = 0;
                self.squid_splashes(coord, facing, rules);
                let ship = self.substrate.entities.get(victim).unwrap();
                let ship_object = self.object_type(ship.type_ref(), rules).unwrap();
                let cull = should_cull(
                    culling,
                    ship.health.current,
                    ship_object.strength,
                    elite,
                    rules.general.condition_red,
                    rules.general.condition_yellow,
                );
                if cull {
                    let eater = self.substrate.entities.get(owner).unwrap();
                    let object = self.object_type(eater.type_ref(), rules).unwrap();
                    let award = (
                        object.trainable,
                        self.cost_of(owner_house, object, rules),
                        self.cost_of(ship.owner(), ship_object, rules),
                    );
                    if award.0 {
                        super::super::veterancy::award_kill(
                            self.substrate.entities.get_mut(owner).unwrap(),
                            award.1,
                            award.2,
                            true,
                            rules.general.veteran_ratio,
                            rules.general.veteran_cap,
                        );
                    }
                    self.parasite_exit_unit(owner, rules, effects);
                    self.record_the_kill(
                        victim,
                        Some(owner),
                        Some(owner_house),
                        crate::sim::combat::KillCallback::Terminal,
                        rules,
                    );
                    self.begin_warp_sinking(victim, rules, effects);
                    if let Some(eater) = self.substrate.entities.get_mut(owner) {
                        eater.paralysis_timer = CdTimer::started(frame as i32, 0);
                    }
                    return;
                }
                let event = crate::sim::combat::EntityDamageEvent::direct_receiver(
                    victim,
                    damage,
                    0,
                    owner,
                    None,
                    self.interner.intern(wh_name),
                    crate::sim::combat::ReceiverCallFlags {
                        ignore_defenses: false,
                        arg6: true,
                    },
                );
                self.commit_direct_damage_receiver(rules, registry, event, effects);
                if self
                    .substrate
                    .entities
                    .get(owner)
                    .and_then(|e| e.parasite.as_deref())
                    .and_then(|p| p.victim)
                    != Some(victim)
                {
                    return;
                }
                self.squid_restart_rock(owner, coord, facing, rules);
            }
            _ => {
                self.parasite_exit_unit(owner, rules, effects);
                return;
            }
        }
        let state = &self
            .substrate
            .entities
            .get(owner)
            .unwrap()
            .parasite
            .as_ref()
            .unwrap()
            .grapple;
        let (anim, roll) = (state.anim, f32::from_bits(state.roll));
        if let Some(ship) = self.substrate.entities.get_mut(victim) {
            let rocking = ship.rocking.get_or_insert_with(RockingState::default);
            rocking.angle_sideways = crate::util::fixed_math::SimFixed::from_num(roll);
            rocking.vel_sideways = crate::util::fixed_math::SimFixed::ZERO;
            rocking.is_ship_rocking = true;
        }
        if let Some(anim) = anim {
            if let Some(stage) = anim_frame {
                self.set_parasite_anim_stage(anim, stage);
            }
            if self
                .anim(anim)
                .is_some_and(|a| a.owner_entity != Some(victim))
            {
                self.set_anim_owner_object(anim, Some(victim), rules);
                self.set_anim_house_remap(anim, owner_house);
                let adjust = self
                    .resolved_terrain
                    .as_ref()
                    .and_then(|g| g.cell((coord.x / 256) as u16, (coord.y / 256) as u16))
                    .map_or(0, |c| {
                        crate::map::lighting::cell_ground_z_adjust(
                            self.scenario_normal_lighting,
                            c.level,
                        )
                    });
                let stage = self.anim(anim).map_or(0, |a| a.runtime.current_frame);
                self.set_anim_frame_and_z_adjust(anim, stage, adjust);
            }
        }
    }
}
#[cfg(test)]
#[path = "squid_grapple_tests.rs"]
mod tests;
