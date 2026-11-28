use crate::openpal3::states::stats::Resources;

use super::{native_integer, timeline::ACTOR_SLOTS};

#[derive(Debug, Clone, PartialEq)]
pub struct CombatVitals {
    pub current: Resources,
    pub maximum: Resources,
    pub enabled: bool,
    pub removed: bool,
    pub invulnerable: bool,
    pub defending: bool,
    pub money_shield: bool,
    pub qi_discount_percent: f32,
    pub mp_discount_percent: f32,
}

impl CombatVitals {
    pub fn new(maximum: Resources, current: Resources) -> Self {
        Self {
            current,
            maximum,
            enabled: true,
            removed: false,
            invulnerable: false,
            defending: false,
            money_shield: false,
            qi_discount_percent: 0.0,
            mp_discount_percent: 0.0,
        }
    }

    /// Native 0x4f2c70 with ordinary lethal cleanup at 0x5136d0.
    /// Healing returns its nominal amount; read HP before/after for actual gain.
    /// Special boss removal/linked-slot policies belong to the battle dispatcher.
    pub fn change_hp(&mut self, mut delta: i32, money: &mut i32) -> i32 {
        if !self.enabled {
            self.current.hp = 0;
            return 0;
        }
        if self.removed || self.invulnerable {
            return 0;
        }
        if delta < -1 && self.defending {
            delta /= 2;
        }
        if delta < 0 && self.money_shield {
            if *money >= delta.wrapping_neg() {
                *money = money.wrapping_add(delta).max(0);
                return 0;
            }
            delta = delta.wrapping_add(*money);
            *money = 0;
        }
        let before = self.current.hp;
        self.current.hp = clamp_resource(before.wrapping_add(delta), self.maximum.hp);
        if self.current.hp <= 0 {
            self.current.hp = 0;
            self.defending = false;
        }
        if delta > 0 {
            delta
        } else {
            before
                .wrapping_sub(self.current.hp)
                .wrapping_abs()
                .wrapping_neg()
        }
    }

    /// Native 0x4f3020: gains are not blocked by invulnerability or removal.
    pub fn change_qi(&mut self, delta: i32) -> i32 {
        if delta == 0 || !self.enabled {
            return 0;
        }
        let delta = discounted_cost(delta, self.qi_discount_percent);
        self.current.qi = clamp_resource(self.current.qi.wrapping_add(delta), self.maximum.qi);
        delta
    }

    /// Native 0x4f31c0: invulnerability blocks gains, but not spending.
    pub fn change_mp(&mut self, delta: i32) -> i32 {
        if delta == 0 || !self.enabled || (self.invulnerable && delta > 0) {
            return 0;
        }
        let delta = discounted_cost(delta, self.mp_discount_percent);
        self.current.mp = clamp_resource(self.current.mp.wrapping_add(delta), self.maximum.mp);
        delta
    }
}

fn clamp_resource(value: i32, maximum: i32) -> i32 {
    if value > maximum {
        maximum
    } else {
        value.max(0)
    }
}

fn discounted_cost(delta: i32, percent: f32) -> i32 {
    if delta >= 0 {
        return delta;
    }
    let discount =
        native_integer((f64::from(delta) * f64::from(percent) * f64::from(0.01f32)) as f32);
    delta.wrapping_sub(discount)
}

#[derive(Debug, Clone, Copy)]
pub enum CombatSide {
    Party,
    Enemies,
}

#[derive(Debug, Clone, Copy)]
pub enum DefeatCheck {
    Health,
    Enabled,
}

/// Native 0x4ed0a0/0x4ed100 distinguish zero HP from completed actor removal.
/// Removed enemies do not prevent completion even if their HP remains positive.
pub fn side_defeated(
    actors: &[Option<CombatVitals>; ACTOR_SLOTS],
    side: CombatSide,
    check: DefeatCheck,
) -> bool {
    let range = match side {
        CombatSide::Party => 0..5,
        CombatSide::Enemies => 5..ACTOR_SLOTS,
    };
    !range.filter_map(|slot| actors[slot].as_ref()).any(|actor| {
        if matches!(side, CombatSide::Enemies) && actor.removed {
            return false;
        }
        match check {
            DefeatCheck::Health => actor.current.hp > 0,
            DefeatCheck::Enabled => actor.enabled,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vitals() -> CombatVitals {
        CombatVitals::new(
            Resources {
                hp: 100,
                qi: 40,
                mp: 50,
            },
            Resources {
                hp: 30,
                qi: 10,
                mp: 20,
            },
        )
    }

    #[test]
    fn defending_precedes_currency_absorption_and_lethal_damage_clamps() {
        let mut actor = vitals();
        actor.defending = true;
        actor.money_shield = true;
        let mut money = 10;
        assert_eq!(actor.change_hp(-31, &mut money), -5);
        assert_eq!((actor.current.hp, money), (25, 0));
        assert_eq!(actor.change_hp(-100, &mut money), -25);
        assert_eq!(actor.current.hp, 0);
        assert!(!actor.defending);
        let mut actor = vitals();
        actor.money_shield = true;
        money = 31;
        assert_eq!(actor.change_hp(-31, &mut money), 0);
        assert_eq!((actor.current.hp, money), (30, 0));
    }

    #[test]
    fn resource_gates_discounts_and_nominal_returns_match_native_rules() {
        let mut actor = vitals();
        let mut money = 0;
        actor.invulnerable = true;
        assert_eq!(actor.change_hp(-30, &mut money), 0);
        assert_eq!(actor.change_mp(31), 0);
        assert_eq!(actor.change_qi(31), 31);
        assert_eq!(actor.current.qi, 40);
        actor.mp_discount_percent = 25.0;
        assert_eq!(actor.change_mp(-31), -23);
        assert_eq!(actor.current.mp, 0);
        actor.invulnerable = false;
        assert_eq!(actor.change_hp(200, &mut money), 200);
        assert_eq!(actor.current.hp, 100);
        actor.removed = true;
        assert_eq!(actor.change_hp(-30, &mut money), 0);
        assert_eq!(actor.change_mp(1), 1);
        actor.enabled = false;
        assert_eq!(actor.change_qi(-10), 0);
        assert_eq!(actor.change_hp(10, &mut money), 0);
        assert_eq!(actor.current.hp, 0);
    }

    #[test]
    fn defeat_checks_preserve_death_animation_and_enemy_removal_distinctions() {
        let mut actors = std::array::from_fn(|_| None);
        let mut party = vitals();
        party.current.hp = 0;
        actors[0] = Some(party);
        assert!(side_defeated(
            &actors,
            CombatSide::Party,
            DefeatCheck::Health
        ));
        assert!(!side_defeated(
            &actors,
            CombatSide::Party,
            DefeatCheck::Enabled
        ));
        let mut enemy = vitals();
        enemy.removed = true;
        actors[5] = Some(enemy);
        assert!(side_defeated(
            &actors,
            CombatSide::Enemies,
            DefeatCheck::Health
        ));
        assert!(side_defeated(
            &actors,
            CombatSide::Enemies,
            DefeatCheck::Enabled
        ));
    }

    #[test]
    #[ignore = "requires PAL3_RESOURCE_ORACLE"]
    fn original_resource_mutations_and_defeat_predicates_match() {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct Input {
            resource: String,
            current: [i32; 3],
            maximum: [i32; 3],
            delta: i32,
            money: i32,
            enabled: bool,
            removed: bool,
            invulnerable: bool,
            defending: bool,
            money_shield: bool,
            qi_discount: f32,
            mp_discount: f32,
        }
        #[derive(Deserialize)]
        struct ResultState {
            current: [i32; 3],
            money: i32,
            defending: bool,
            enabled: bool,
            removed: bool,
            returned: i32,
        }
        #[derive(Deserialize)]
        struct Case {
            input: Input,
            result: ResultState,
        }
        #[derive(Deserialize)]
        struct Actor {
            hp: i32,
            enabled: bool,
            removed: bool,
        }
        #[derive(Deserialize)]
        struct Outcome {
            actors: [Option<Actor>; ACTOR_SLOTS],
            result: [bool; 4],
        }
        #[derive(Deserialize)]
        struct Oracle {
            cases: Vec<Case>,
            outcomes: Vec<Outcome>,
        }
        fn resources(values: [i32; 3]) -> Resources {
            Resources {
                hp: values[0],
                qi: values[1],
                mp: values[2],
            }
        }
        let oracle: Oracle = serde_json::from_slice(
            &std::fs::read(std::env::var("PAL3_RESOURCE_ORACLE").unwrap()).unwrap(),
        )
        .unwrap();
        assert!(!oracle.cases.is_empty() && !oracle.outcomes.is_empty());
        for (index, case) in oracle.cases.iter().enumerate() {
            let input = &case.input;
            let mut actor = CombatVitals::new(resources(input.maximum), resources(input.current));
            actor.enabled = input.enabled;
            actor.removed = input.removed;
            actor.invulnerable = input.invulnerable;
            actor.defending = input.defending;
            actor.money_shield = input.money_shield;
            actor.qi_discount_percent = input.qi_discount;
            actor.mp_discount_percent = input.mp_discount;
            let mut money = input.money;
            let returned = match input.resource.as_str() {
                "hp" => actor.change_hp(input.delta, &mut money),
                "qi" => actor.change_qi(input.delta),
                "mp" => actor.change_mp(input.delta),
                _ => panic!("Unknown native resource"),
            };
            assert_eq!(
                actor.current,
                resources(case.result.current),
                "resource case {index}"
            );
            assert_eq!(
                (
                    money,
                    actor.defending,
                    actor.enabled,
                    actor.removed,
                    returned
                ),
                (
                    case.result.money,
                    case.result.defending,
                    case.result.enabled,
                    case.result.removed,
                    case.result.returned
                ),
                "flags/return case {index}"
            );
        }
        for (index, outcome) in oracle.outcomes.iter().enumerate() {
            let actors = std::array::from_fn(|slot| {
                outcome.actors[slot].as_ref().map(|input| {
                    let mut actor = vitals();
                    actor.current.hp = input.hp;
                    actor.enabled = input.enabled;
                    actor.removed = input.removed;
                    actor
                })
            });
            let actual = [
                side_defeated(&actors, CombatSide::Party, DefeatCheck::Health),
                side_defeated(&actors, CombatSide::Party, DefeatCheck::Enabled),
                side_defeated(&actors, CombatSide::Enemies, DefeatCheck::Health),
                side_defeated(&actors, CombatSide::Enemies, DefeatCheck::Enabled),
            ];
            assert_eq!(actual, outcome.result, "defeat case {index}");
        }
        eprintln!(
            "Compared {} native resource cases and {} defeat configurations",
            oracle.cases.len(),
            oracle.outcomes.len()
        );
    }
}
