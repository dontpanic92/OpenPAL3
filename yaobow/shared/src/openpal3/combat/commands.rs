use super::{
    battle::{AttackError, AttackReport, BattleState},
    timeline::{ACTOR_SLOTS, READY_THRESHOLD},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    Attack(AttackError),
    NotReady,
    Busy,
    UnsupportedMultiHit,
    InvalidClock,
    UnknownAction,
    NotStarted,
    HitAlreadyConsumed,
    IdExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandEvent {
    Started {
        id: ActionId,
        source: usize,
        target: usize,
    },
    Cancelled {
        id: ActionId,
        reason: AttackError,
    },
}

#[derive(Clone, Copy)]
struct PendingAttack {
    id: ActionId,
    source: usize,
    target: usize,
    started: bool,
    hit_consumed: bool,
}

/// Ordinary single-hit command flow, with presentation-driven hit/completion
/// signals. Native melee has no spell-preparation delay to approximate.
pub struct BattleCommands {
    battle: BattleState,
    queue: [Option<PendingAttack>; ACTOR_SLOTS],
    next_id: u64,
}

impl BattleCommands {
    pub fn new(battle: BattleState) -> Self {
        Self {
            battle,
            queue: [None; ACTOR_SLOTS],
            next_id: 0,
        }
    }

    pub fn battle(&self) -> &BattleState {
        &self.battle
    }

    pub fn set_running(&mut self, running: bool) {
        self.battle.timeline_mut().set_running(running);
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.battle.timeline_mut().set_paused(paused);
    }

    pub fn ready_actors(&self) -> impl Iterator<Item = usize> + '_ {
        (0..ACTOR_SLOTS).filter(|&source| {
            self.battle.actor(source).is_some_and(|actor| {
                actor.vitals.enabled && !actor.vitals.removed && actor.vitals.current.hp > 0
            }) && self.battle.timeline().meter(source).is_some_and(|meter| {
                meter.ready >= READY_THRESHOLD && !meter.preparing && !meter.held()
            }) && !self
                .queue
                .iter()
                .flatten()
                .any(|command| command.source == source)
        })
    }

    pub fn queue_attack(&mut self, source: usize, target: usize) -> Result<ActionId, CommandError> {
        self.battle
            .validate_physical(source, target)
            .map_err(CommandError::Attack)?;
        if !self.battle.actor(target).unwrap().vitals.enabled {
            return Err(CommandError::Attack(AttackError::InvalidTarget));
        }
        if source < 5 && self.battle.actor(source).unwrap().hit_divisor > 1 {
            return Err(CommandError::UnsupportedMultiHit);
        }
        if self
            .queue
            .iter()
            .flatten()
            .any(|command| command.source == source)
        {
            return Err(CommandError::Busy);
        }
        let meter = self.battle.timeline().meter(source).unwrap();
        if meter.ready < READY_THRESHOLD || meter.preparing || meter.held() {
            return Err(CommandError::NotReady);
        }
        let next = self
            .next_id
            .checked_add(1)
            .ok_or(CommandError::IdExhausted)?;
        let slot = self.queue.iter().position(Option::is_none).unwrap();
        self.next_id = next;
        let id = ActionId(next);
        self.queue[slot] = Some(PendingAttack {
            id,
            source,
            target,
            started: false,
            hit_consumed: false,
        });
        Ok(id)
    }

    /// Native command handlers run before the meter update (0x513060).
    /// Starting melee holds the actor and clears both meters (0x513fd0).
    pub fn advance_to(&mut self, now: f32) -> Result<Vec<CommandEvent>, CommandError> {
        if !now.is_finite() || now < self.battle.timeline().time() {
            return Err(CommandError::InvalidClock);
        }
        let mut events = vec![];
        for slot in 0..ACTOR_SLOTS {
            let Some(mut command) = self.queue[slot] else {
                continue;
            };
            if command.started {
                continue;
            }
            if let Err(reason) = self
                .battle
                .validate_physical(command.source, command.target)
            {
                self.queue[slot] = None;
                events.push(CommandEvent::Cancelled {
                    id: command.id,
                    reason,
                });
                continue;
            }
            self.battle.timeline_mut().hold_actor(command.source, true);
            self.battle.timeline_mut().reset_progress(command.source);
            command.started = true;
            self.queue[slot] = Some(command);
            events.push(CommandEvent::Started {
                id: command.id,
                source: command.source,
                target: command.target,
            });
        }
        let enabled = std::array::from_fn(|slot| {
            self.battle
                .actor(slot)
                .is_some_and(|actor| actor.vitals.enabled)
        });
        assert!(self.battle.timeline_mut().advance_to(now, enabled));
        Ok(events)
    }

    /// The animation host calls this on the game thread at the native hit marker.
    /// IDs prevent late/duplicate callbacks from damaging a subsequent action.
    pub fn hit(&mut self, id: ActionId) -> Result<AttackReport, CommandError> {
        let slot = self.find(id)?;
        let mut command = self.queue[slot].unwrap();
        if !command.started {
            return Err(CommandError::NotStarted);
        }
        if command.hit_consumed {
            return Err(CommandError::HitAlreadyConsumed);
        }
        command.hit_consumed = true;
        self.queue[slot] = Some(command);
        self.battle
            .resolve_physical(command.source, command.target)
            .map_err(CommandError::Attack)
    }

    /// Native group completion releases the actor without resetting meters again.
    /// A completed/aborted animation need not have produced a hit.
    pub fn complete(&mut self, id: ActionId) -> Result<(), CommandError> {
        let slot = self.find(id)?;
        let command = self.queue[slot].unwrap();
        if !command.started {
            return Err(CommandError::NotStarted);
        }
        self.battle.timeline_mut().hold_actor(command.source, false);
        self.queue[slot] = None;
        Ok(())
    }

    pub fn cancel(&mut self, id: ActionId) -> Result<(), CommandError> {
        let slot = self.find(id)?;
        let command = self.queue[slot].unwrap();
        if command.started {
            self.battle.timeline_mut().hold_actor(command.source, false);
        }
        self.queue[slot] = None;
        Ok(())
    }

    fn find(&self, id: ActionId) -> Result<usize, CommandError> {
        self.queue
            .iter()
            .position(|command| command.is_some_and(|command| command.id == id))
            .ok_or(CommandError::UnknownAction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openpal3::{
        combat::{
            battle::{AttackOutcome, PhysicalActor},
            timeline::Opening,
        },
        states::stats::{Attributes, Resources},
    };

    fn battle() -> BattleState {
        let actor = || {
            let mut actor = PhysicalActor::new(
                20,
                Attributes {
                    hp: 10000,
                    qi: 100,
                    mp: 100,
                    attack: 350,
                    defense: 30,
                    speed: 3,
                    luck: 2,
                    ..Default::default()
                },
                Resources {
                    hp: 10000,
                    qi: 0,
                    mp: 100,
                },
            );
            actor.normal_attack = 924;
            actor.qi_gain = 8;
            actor
        };
        let mut actors = std::array::from_fn(|_| None);
        actors[0] = Some(actor());
        actors[5] = Some(actor());
        BattleState::new(actors, Opening::PartyReady, 1, 0)
    }

    #[test]
    fn timed_turns_apply_damage_only_at_the_marker_and_reject_duplicate_callbacks() {
        let mut commands = BattleCommands::new(battle());
        commands.set_running(true);
        let id = commands.queue_attack(0, 5).unwrap();
        assert_eq!(commands.hit(id), Err(CommandError::NotStarted));
        assert_eq!(commands.queue_attack(0, 5), Err(CommandError::Busy));
        assert_eq!(
            commands.advance_to(0.125).unwrap(),
            [CommandEvent::Started {
                id,
                source: 0,
                target: 5
            },]
        );
        assert_eq!(commands.battle().actor(5).unwrap().vitals.current.hp, 10000);
        commands.advance_to(1.0).unwrap();
        assert_eq!(commands.battle().timeline().meter(0).unwrap().ready, 0);
        assert_eq!(commands.hit(id).unwrap().outcome, AttackOutcome::Hit);
        let hp = commands.battle().actor(5).unwrap().vitals.current.hp;
        let rng = *commands.battle().rng();
        assert_eq!(commands.hit(id), Err(CommandError::HitAlreadyConsumed));
        assert_eq!(*commands.battle().rng(), rng);
        assert_eq!(commands.battle().actor(5).unwrap().vitals.current.hp, hp);
        commands.complete(id).unwrap();
        commands.advance_to(1.125).unwrap();
        assert_eq!(commands.battle().timeline().meter(0).unwrap().ready, 218);
        assert_eq!(commands.hit(id), Err(CommandError::UnknownAction));
    }

    #[test]
    fn clock_errors_and_cancelled_or_reused_slots_do_not_execute_stale_commands() {
        let mut commands = BattleCommands::new(battle());
        commands.set_running(true);
        let old = commands.queue_attack(0, 5).unwrap();
        assert_eq!(
            commands.advance_to(f32::NAN),
            Err(CommandError::InvalidClock)
        );
        assert!(!commands.battle().timeline().meter(0).unwrap().held());
        commands.cancel(old).unwrap();
        let new = commands.queue_attack(0, 5).unwrap();
        assert_ne!(old, new);
        commands.advance_to(0.125).unwrap();
        assert_eq!(commands.complete(old), Err(CommandError::UnknownAction));
        commands.complete(new).unwrap();
        assert_eq!(commands.battle().actor(5).unwrap().vitals.current.hp, 10000);
        assert_eq!(commands.queue_attack(0, 5), Err(CommandError::NotReady));
    }

    #[test]
    fn headless_host_exchanges_timed_turns_without_bypassing_readiness() {
        let mut actor = battle().actor(0).unwrap().clone();
        actor.vitals.current.hp = 800;
        let mut actors = std::array::from_fn(|_| None);
        actors[0] = Some(actor.clone());
        actors[5] = Some(actor);
        let mut commands = BattleCommands::new(BattleState::new(actors, Opening::PartyReady, 1, 0));
        commands.set_running(true);
        let mut hits = 0;
        for frame in 1..=2000 {
            let ready: Vec<_> = commands.ready_actors().collect();
            for source in ready {
                let target = if source == 0 { 5 } else { 0 };
                if commands.battle().actor(target).unwrap().vitals.current.hp > 0 {
                    commands.queue_attack(source, target).unwrap();
                }
            }
            let events = commands.advance_to(frame as f32 / 60.0).unwrap();
            // This test host supplies animation signals immediately; the game
            // renderer must supply real markers/completion, not guessed delays.
            for event in events {
                if let CommandEvent::Started { id, .. } = event {
                    if commands.hit(id).is_ok() {
                        hits += 1;
                    }
                    commands.complete(id).unwrap();
                }
            }
            if [0, 5]
                .iter()
                .any(|&slot| commands.battle().actor(slot).unwrap().vitals.current.hp == 0)
            {
                break;
            }
        }
        assert!(hits >= 3);
        assert!(
            [0, 5]
                .iter()
                .all(|&slot| commands.battle().actor(slot).unwrap().vitals.current.hp < 800)
        );
        assert!(
            [0, 5]
                .iter()
                .any(|&slot| commands.battle().actor(slot).unwrap().vitals.current.hp == 0)
        );
    }

    #[test]
    #[ignore = "requires PAL3_COMMAND_ORACLE"]
    fn original_melee_start_and_completion_match_meter_transitions() {
        use serde::Deserialize;
        #[derive(Deserialize)]
        struct Input {
            source: usize,
            ready: i32,
            action: i32,
            held: bool,
            done: bool,
        }
        #[derive(Deserialize)]
        struct ResultState {
            ready: i32,
            action: i32,
            preparing: bool,
            held: bool,
            active: bool,
            callbacks: Vec<u32>,
        }
        #[derive(Deserialize)]
        struct Case {
            input: Input,
            result: ResultState,
        }
        #[derive(Deserialize)]
        struct Oracle {
            cases: Vec<Case>,
        }
        let oracle: Oracle = serde_json::from_slice(
            &std::fs::read(std::env::var("PAL3_COMMAND_ORACLE").unwrap()).unwrap(),
        )
        .unwrap();
        assert!(!oracle.cases.is_empty());
        for (index, case) in oracle.cases.iter().enumerate() {
            let mut battle = battle();
            // These fixtures exercise the handler after command admission; its
            // meter reset is unconditional even for non-ready raw input.
            let source = case.input.source;
            let target = if source < 5 { 5 } else { 0 };
            let template = battle.actor(0).unwrap().clone();
            // Rebuild the roster to cover all native party/enemy slot boundaries.
            let mut actors = std::array::from_fn(|_| None);
            actors[source] = Some(template.clone());
            actors[target] = Some(template);
            battle = BattleState::new(actors, Opening::Normal, 1, 0);
            battle
                .timeline_mut()
                .set_progress(source, case.input.ready, case.input.action, true);
            battle.timeline_mut().hold_actor(source, case.input.held);
            let mut commands = BattleCommands::new(battle);
            let id = ActionId(1);
            commands.queue[0] = Some(PendingAttack {
                id,
                source,
                target,
                started: false,
                hit_consumed: false,
            });
            commands.advance_to(0.0).unwrap();
            if case.input.done {
                commands.complete(id).unwrap();
            }
            let meter = commands.battle().timeline().meter(source).unwrap();
            assert_eq!(
                (
                    meter.ready,
                    meter.action,
                    meter.preparing,
                    meter.held(),
                    commands.find(id).is_ok()
                ),
                (
                    case.result.ready,
                    case.result.action,
                    case.result.preparing,
                    case.result.held,
                    case.result.active
                ),
                "native command {index}"
            );
            assert_eq!(
                case.result.callbacks,
                [0x4fe550, 0x4fe660, 0x4fece0, 0x4fefb0, 0x4fe600,],
                "native callback sequence {index}"
            );
        }
        eprintln!(
            "Compared {} native ordinary-command lifecycle cases",
            oracle.cases.len()
        );
    }
}
