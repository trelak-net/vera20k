//! Tesla charger ownership: ElectricAssault detonation (46937A), registration
//! (452820), Building UpdateOverpower (450590, inlined at 44010D), and
//! Building PointerExpired (44EA77). Target selection already admits the
//! Trooper's allied Coil order and chooses the ElectricAssault secondary.
use crate::map::entities::EntityCategory;
use crate::rules::ruleset::RuleSet;
use crate::sim::{combat::TargetKind, world::Simulation};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct TeslaCharging {
    /// Building+66C: successful chargers, in last-charge order.
    chargers: Vec<u64>,
    /// Building+661: the latch sampled by SelectWeapon, updated at the Coil's
    /// own AI tail. Registration/expiry do not update it early.
    overpowered: bool,
}
impl TeslaCharging {
    pub(crate) fn count(&self) -> i32 {
        self.chargers.len() as i32
    }
    pub(crate) fn is_overpowered(&self) -> bool {
        self.overpowered
    }
    pub(crate) fn references(&self) -> impl Iterator<Item = u64> + '_ {
        self.chargers.iter().copied()
    }
    pub(crate) fn names(&self, id: u64) -> bool {
        self.chargers.contains(&id)
    }
    fn register(&mut self, id: u64) {
        self.forget(id);
        self.chargers.push(id);
    }
    pub(crate) fn forget(&mut self, id: u64) {
        self.chargers.retain(|&charger| charger != id);
    }
    fn update(&mut self, targets_coil: impl FnMut(&u64) -> bool, online: bool, full_power: bool) {
        self.chargers.retain(targets_coil);
        self.overpowered =
            self.chargers.len() >= 3 || (!self.chargers.is_empty() && online && full_power);
    }
}
impl Simulation {
    pub(crate) fn can_charge_tesla(&self, firer: u64, target: u64, rules: &RuleSet) -> bool {
        let (Some(actor), Some(coil)) = (self.entities().get(firer), self.entities().get(target))
        else {
            return false;
        };
        if actor.category != EntityCategory::Infantry
            || coil.category != EntityCategory::Structure
            || !crate::map::houses::are_houses_friendly(
                &self.house_alliances,
                self.interner.resolve(actor.owner()),
                self.interner.resolve(coil.owner()),
            )
        {
            return false;
        }
        let (Some(actor_type), Some(coil_type)) = (
            self.object_type(actor.type_ref(), rules),
            self.object_type(coil.type_ref(), rules),
        ) else {
            return false;
        };
        coil_type.overpowerable
            && crate::sim::combat::combat_weapon::get_weapon(
                actor,
                actor_type,
                1,
                self.entities(),
                rules,
                &self.interner,
            )
            .and_then(|weapon| weapon.warhead.as_deref())
            .and_then(|name| rules.warhead(name))
            .is_some_and(|warhead| warhead.electric_assault)
    }

    pub(crate) fn tesla_charge_detonation(&mut self, firer: u64, target: Option<u64>) {
        // The original arm tests only these classes; allegiance and weapon
        // admission belong to SelectWeapon/GetFireError, not registration.
        if !self
            .entities()
            .get(firer)
            .is_some_and(|entity| entity.category == EntityCategory::Infantry)
        {
            return;
        }
        let Some(target) = target else {
            return;
        };
        if let Some(coil) = self.substrate.entities.get_mut(target)
            && coil.category == EntityCategory::Structure
        {
            coil.tesla_charging.register(firer);
        }
    }
    pub(crate) fn update_tesla_charging(&mut self, id: u64, rules: &RuleSet) {
        let Some(coil) = self.entities().get(id) else {
            return;
        };
        if coil.category != EntityCategory::Structure
            || !self
                .object_type(coil.type_ref(), rules)
                .is_some_and(|kind| kind.overpowerable)
        {
            return;
        }
        let online = coil.building_online();
        let full_power = self
            .power_states
            .get(&coil.owner())
            .is_none_or(|power| power.has_full_power());
        let mut charging =
            std::mem::take(&mut self.substrate.entities.get_mut(id).unwrap().tesla_charging);
        charging.update(
            |charger| {
                self.entities().get(*charger).is_some_and(|entity| {
                    entity
                        .attack_target
                        .as_ref()
                        .is_some_and(|attack| attack.target == TargetKind::Entity(id))
                })
            },
            online,
            full_power,
        );
        self.substrate.entities.get_mut(id).unwrap().tesla_charging = charging;
    }
    pub(crate) fn tesla_pointer_expired(&mut self, listener: u64, expired: u64) {
        if self
            .entities()
            .get(listener)
            .is_some_and(|entity| entity.tesla_charging.names(expired))
        {
            self.substrate
                .entities
                .get_mut(listener)
                .unwrap()
                .tesla_charging
                .forget(expired);
        }
    }
}

#[cfg(test)]
#[path = "tesla_charging_tests.rs"]
pub(crate) mod tests;
