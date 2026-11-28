pub mod ai;
pub mod battle;
pub mod commands;
pub mod resources;
pub mod timeline;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatRng {
    state: u32,
    draws: u32,
}

impl CombatRng {
    pub fn new(seed: u32) -> Self {
        Self {
            state: seed,
            draws: 0,
        }
    }

    pub fn state(&self) -> u32 {
        self.state
    }

    pub fn draws(&self) -> u32 {
        self.draws
    }

    fn next_u15(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(214013).wrapping_add(2531011);
        self.draws = self.draws.wrapping_add(1);
        (self.state >> 16) & 0x7fff
    }
}

fn native_integer(value: f32) -> i32 {
    let rounded = f64::from(value).round_ties_even();
    if !(-2147483648.0..2147483648.0).contains(&rounded) {
        // x87 FISTP returns integer-indefinite on an invalid conversion.
        i32::MIN
    } else {
        rounded as i32
    }
}

/// Native 0x4f37a0/0x4f3820/0x4f38a0/0x4f3920 stat modifiers.
/// An increase takes precedence if both opposing flags are present.
pub fn modified_attribute(base: i32, increased: bool, decreased: bool) -> i32 {
    let factor = if increased {
        0.25
    } else if decreased {
        -0.25
    } else {
        return base;
    };
    base.wrapping_add(native_integer((f64::from(base) * factor) as f32))
}

/// Base signed physical effect from 0x4f98b0 case 7; negative means damage.
/// Uses effective attack/defense. Attack-pattern and elemental adjustments,
/// hit eligibility and resource application belong to the action resolver.
pub fn physical_base_delta(
    level: i32,
    attack: i32,
    defense: i32,
    rng: Option<&mut CombatRng>,
) -> i32 {
    let factor = 1.0 + f64::from(level) * f64::from(0.005f32);
    let mut delta =
        native_integer((f64::from(attack.wrapping_sub(defense)) * factor) as f32).wrapping_neg();
    if let Some(rng) = rng {
        let spread = native_integer((f64::from(delta) * f64::from(0.1f32)) as f32).unsigned_abs();
        if spread != 0 {
            let offset = (rng.next_u15() % spread) as i32;
            delta = if rng.next_u15() % 1024 >= 512 {
                delta.wrapping_add(offset)
            } else {
                delta.wrapping_sub(offset)
            };
        }
    }
    native_integer(delta as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_effect_preserves_native_rounding_and_signed_results() {
        assert_eq!(physical_base_delta(50, 350, 80, None), -338);
        assert_eq!(physical_base_delta(50, 350, 84, None), -332);
        assert_eq!(physical_base_delta(20, 30, 30, None), 0);
        assert_eq!(physical_base_delta(20, 30, 60, None), 33);
        assert_eq!(modified_attribute(6, true, false), 8);
        assert_eq!(modified_attribute(6, false, true), 4);
        assert_eq!(modified_attribute(6, true, true), 8);
    }

    #[test]
    fn physical_variance_preserves_native_draw_count_and_exclusive_spread() {
        let mut rng = CombatRng::new(1);
        assert_eq!(physical_base_delta(1, 5, 0, Some(&mut rng)), -5);
        assert_eq!((rng.state(), rng.draws()), (1, 0));
        for seed in 0..64 {
            let mut rng = CombatRng::new(seed);
            let delta = physical_base_delta(20, 350, 30, Some(&mut rng));
            assert!((-386..=-318).contains(&delta));
            assert_eq!(rng.draws(), 2);
            let mut replay = CombatRng::new(seed);
            assert_eq!(physical_base_delta(20, 350, 30, Some(&mut replay)), delta);
            assert_eq!(replay, rng);
        }
    }

    #[test]
    #[ignore = "requires original PAL3 assets and PAL3_COMBAT_ORACLE"]
    fn original_role_initialization_and_physical_effects_match() {
        use crate::openpal3::loaders::stats::StatsData;
        use common::store_ext::StoreExt2;
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct Role {
            id: u32,
            level: i32,
            attributes: [i32; 7],
            experience: i32,
            normal_attack: u32,
            qi_gain: i32,
        }
        #[derive(Deserialize)]
        struct Physical {
            level: i32,
            attack: i32,
            defense: i32,
            seed: u32,
            random: bool,
            attack_flags: u8,
            defense_flags: u8,
            effective_attack: i32,
            effective_defense: i32,
            delta: i32,
            next_seed: u32,
            draws: u32,
        }
        #[derive(Deserialize)]
        struct Oracle {
            roles: Vec<Role>,
            physical: Vec<Physical>,
        }
        let vfs = packfs::init_virtual_fs(std::env::var("PAL3_ASSET_PATH").unwrap(), None);
        let data = StatsData::load(
            &vfs.read_to_end("/basedata/basedata/cbdata/PAL3_Softstar.gdb")
                .unwrap(),
            &vfs.read_to_end_from_gbk("/basedata/basedata/datascript/combat.ini")
                .unwrap(),
        )
        .unwrap();
        let oracle: Oracle = serde_json::from_slice(
            &std::fs::read(std::env::var("PAL3_COMBAT_ORACLE").unwrap()).unwrap(),
        )
        .unwrap();
        assert!(!oracle.roles.is_empty() && !oracle.physical.is_empty());
        for role in &oracle.roles {
            let actual = data.combat_role(role.id).unwrap();
            assert_eq!(actual.level, role.level, "role {}", role.id);
            assert_eq!(actual.attributes[..7], role.attributes, "role {}", role.id);
            assert_eq!(
                (actual.experience, actual.normal_attack, actual.qi_gain),
                (role.experience, role.normal_attack, role.qi_gain),
                "role {}",
                role.id
            );
        }
        for (index, case) in oracle.physical.iter().enumerate() {
            let attack = modified_attribute(
                case.attack,
                case.attack_flags & 1 != 0,
                case.attack_flags & 2 != 0,
            );
            let defense = modified_attribute(
                case.defense,
                case.defense_flags & 1 != 0,
                case.defense_flags & 2 != 0,
            );
            assert_eq!(attack, case.effective_attack, "attack case {index}");
            assert_eq!(defense, case.effective_defense, "defense case {index}");
            let mut rng = CombatRng::new(case.seed);
            let delta =
                physical_base_delta(case.level, attack, defense, case.random.then_some(&mut rng));
            assert_eq!(
                (delta, rng.state(), rng.draws()),
                (case.delta, case.next_seed, case.draws),
                "physical case {index}"
            );
        }
        eprintln!(
            "Compared {} native role initializations and {} physical effects",
            oracle.roles.len(),
            oracle.physical.len()
        );
    }
}
