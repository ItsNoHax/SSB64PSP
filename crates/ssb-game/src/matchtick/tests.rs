//! The order [`super::step`] runs the frame in, against the decomp's
//! process table (`gcRunAll`: priority 5 to 0, then link id, then link
//! order), with every departure named.

use super::*;
use crate::fighter::FighterKind;
use crate::weapon::MapSurface;

/// One process of the decomp's battle frame: priority, link id, where it
/// is registered.
struct Process {
    phase: fn(u8) -> Phase,
    per_fighter: bool,
    priority: u8,
    link: u8,
    #[allow(dead_code)]
    decomp: &'static str,
}

/// The battle's processes (`objdef.h` link ids; registrations).
const PROCESSES: &[Process] = &[
    Process {
        phase: |_| Phase::GroundAnim,
        per_fighter: false,
        priority: 5,
        link: 1,
        decomp: "grcommon gr*.c gcPlayAnimAll (grzebes.c:94)",
    },
    Process {
        phase: Phase::Interrupt,
        per_fighter: true,
        priority: 5,
        link: 3,
        decomp: "ftmanager.c:858 ftMainProcUpdateInterrupt",
    },
    Process {
        phase: |_| Phase::StageController,
        per_fighter: false,
        priority: 4,
        link: 1,
        decomp: "grdisplay.c:209,214 gr*ProcUpdate",
    },
    Process {
        phase: Phase::Physics,
        per_fighter: true,
        priority: 4,
        link: 3,
        decomp: "ftmanager.c:859 ftMainProcPhysicsMapDefault",
    },
    Process {
        phase: |_| Phase::ItemActor,
        per_fighter: false,
        priority: 3,
        link: 2,
        decomp: "itmanager.c:593 itManagerAppearActorProcUpdate",
    },
    Process {
        phase: Phase::Capture,
        per_fighter: true,
        priority: 3,
        link: 3,
        decomp: "ftmanager.c:860 ftMainProcPhysicsMapCapture",
    },
    Process {
        phase: |_| Phase::Items,
        per_fighter: false,
        priority: 3,
        link: 4,
        decomp: "itmanager.c:415 itProcessProcItemMain",
    },
    Process {
        phase: |_| Phase::Weapons,
        per_fighter: false,
        priority: 3,
        link: 5,
        decomp: "wpmanager.c:304 wpProcessProcWeaponMain",
    },
    Process {
        phase: |_| Phase::Effects,
        per_fighter: false,
        priority: 3,
        link: 6,
        decomp: "efmanager.c:1920 effect proc_update",
    },
    Process {
        phase: |_| Phase::Camera,
        per_fighter: false,
        priority: 3,
        link: 9,
        decomp: "gmcamera.c:1113 gmCameraRunFuncCamera",
    },
    Process {
        phase: Phase::Catch,
        per_fighter: true,
        priority: 2,
        link: 3,
        decomp: "ftmanager.c:861 ftMainProcSearchCatch",
    },
    // ftmanager.c:862, itmanager.c:416, wpmanager.c:305: one phase here.
    Process {
        phase: |_| Phase::Hits,
        per_fighter: false,
        priority: 1,
        link: 3,
        decomp: "ftmanager.c:862 ftMainProcSearchHitAll",
    },
    Process {
        phase: |_| Phase::Params,
        per_fighter: false,
        priority: 0,
        link: 3,
        decomp: "ftmanager.c:863 ftMainProcParams",
    },
    Process {
        phase: |_| Phase::ItemCollisions,
        per_fighter: false,
        priority: 0,
        link: 4,
        decomp: "itmanager.c:417 itProcessProcHitCollisions",
    },
    Process {
        phase: |_| Phase::WeaponCollisions,
        per_fighter: false,
        priority: 0,
        link: 5,
        decomp: "wpmanager.c:306 wpProcessProcHitCollisions",
    },
];

/// The decomp's order for `seats` fighters, `held` of them in the capture
/// pass.
fn decomp_order(seats: u8, held: &[u8]) -> Vec<Phase> {
    let mut table: Vec<&Process> = PROCESSES.iter().collect();
    table.sort_by_key(|p| (core::cmp::Reverse(p.priority), p.link));
    let mut out = Vec::new();
    for p in table {
        if !p.per_fighter {
            out.push((p.phase)(0));
            continue;
        }
        for i in 0..seats {
            let capture = held.contains(&i);
            let runs = match (p.phase)(i) {
                Phase::Physics(_) => !capture,
                Phase::Capture(_) => capture,
                _ => true,
            };
            if runs {
                out.push((p.phase)(i));
            }
        }
    }
    out
}

/// [`decomp_order`] with the port's departures, each named.
fn port_order(seats: u8, held: &[u8]) -> Vec<Phase> {
    let mut order = decomp_order(seats, held);
    // G1: the appearance actor (ItemActor link 2, priority 3) runs after
    // the capture pass (Fighter link 3, priority 3).
    let actor = order.iter().position(|&p| p == Phase::ItemActor).unwrap();
    order.remove(actor);
    let items = order.iter().position(|&p| p == Phase::Items).unwrap();
    order.insert(items, Phase::ItemActor);
    // G6: the weapons' priority-0 hit-collision effects before the items'
    // priority-0 process.
    let item = order
        .iter()
        .position(|&p| p == Phase::ItemCollisions)
        .unwrap();
    order.swap(item, item + 1);
    // The port's first-frame settle of the statuses the hit phase set: no
    // process of its own (D-052).
    order.push(Phase::Settle);
    order
}

struct NoGround;

impl Ground for NoGround {
    type Surfaces<'a> = core::iter::Empty<MapSurface>;
    fn surfaces(&self) -> Self::Surfaces<'_> {
        core::iter::empty()
    }
    fn groups(&self) -> &[MapGroup] {
        &[]
    }
    fn blast_zone(&self) -> BlastZone {
        BlastZone {
            top: 10000.0,
            bottom: -10000.0,
            left: -10000.0,
            right: 10000.0,
        }
    }
    fn quake_translate(&self, _: u8, _: u16) -> Option<(f32, f32)> {
        None
    }
}

struct NoProps;
impl StageObjects for NoProps {}
impl ItemAnims for NoProps {}

struct Bare(NoGround, NoProps);

impl Arena for Bare {
    type Ground = NoGround;
    type Props = NoProps;
    fn ground(&self) -> &NoGround {
        &self.0
    }
    fn split(&mut self) -> (&NoGround, &mut NoProps) {
        (&self.0, &mut self.1)
    }
}

impl Seat<NoGround> for Fighter {
    fn interrupt(&mut self, _: &NoGround, _: &InputView<'_>) {}
    fn physics(&mut self, _: &NoGround) {}
    fn settle_catch(&mut self, _: &NoGround) {}
    fn settle_capture(&mut self, _: &NoGround) {}
    fn settle_status(&mut self, _: &NoGround) {}
}

#[derive(Default)]
struct Trace(Vec<Phase>);

impl MatchHooks<Fighter> for Trace {
    fn phase(&mut self, phase: Phase) {
        self.0.push(phase);
    }
}

/// One frame over `seats` fighters, `held` of them held (capture pass).
fn trace(seats: u8, held: &[u8]) -> Vec<Phase> {
    let mut fighters: Vec<Fighter> = (0..seats)
        .map(|i| {
            let mut f = Fighter::new(FighterKind::Mario, i, 3);
            f.pos.x = f32::from(i) * 1000.0;
            f
        })
        .collect();
    for &h in held {
        let f = &mut fighters[usize::from(h)];
        f.grab.capture = Some(if h == 0 { 1 } else { 0 });
        f.grab.is_catch_or_capture = false;
    }
    let mut weapons: Box<WeaponPool> = Box::default();
    let mut items: Box<ItemPool> = Box::default();
    let mut stage = Stage::none();
    let mut effects = crate::effect::NoEffects;
    let mut world = World {
        weapons: &mut weapons,
        items: &mut items,
        stage: &mut stage,
        effects: &mut effects,
        bonus: None,
        battle: None,
        started: true,
        locked: false,
    };
    let mut it = fighters.iter_mut();
    let mut seats: Seats<'_, Fighter> = [it.next(), it.next(), it.next(), it.next()];
    let mut hooks = Trace::default();
    step(
        &mut Bare(NoGround, NoProps),
        &mut seats,
        &mut world,
        &mut hooks,
    );
    hooks.0
}

#[test]
fn step_runs_the_decomp_process_order_with_the_named_departures() {
    for seats in 1..=4u8 {
        assert_eq!(
            trace(seats, &[]),
            port_order(seats, &[]),
            "{seats} fighters"
        );
    }
    // A held fighter runs in the priority-3 capture pass, after the others'
    // priority-4 physics.
    assert_eq!(trace(2, &[0]), port_order(2, &[0]));
    assert_eq!(trace(4, &[1, 3]), port_order(4, &[1, 3]));
}

#[test]
fn the_decomp_table_sorts_by_priority_then_link() {
    let order = decomp_order(2, &[1]);
    assert_eq!(
        &order[..8],
        &[
            Phase::GroundAnim,
            Phase::Interrupt(0),
            Phase::Interrupt(1),
            Phase::StageController,
            Phase::Physics(0),
            Phase::ItemActor,
            Phase::Capture(1),
            Phase::Items,
        ]
    );
    // The port's departures are departures: the table without them would
    // not match the trace.
    assert_ne!(trace(2, &[1]), {
        let mut o = decomp_order(2, &[1]);
        o.push(Phase::Settle);
        o
    });
}

#[test]
fn exchange_reaches_the_partner_before_the_first_other() {
    let mut fighters: Vec<Fighter> = (0..3)
        .map(|i| Fighter::new(FighterKind::Mario, i, 3))
        .collect();
    fighters[0].grab.catch = Some(2);
    let mut it = fighters.iter_mut();
    let seats: Seats<'_, Fighter> = [it.next(), it.next(), it.next(), None];
    assert_eq!(index_of(&seats, 2), Some(2));
    assert_eq!(crate::grab::partner(seats[0].as_deref().unwrap()), Some(2));
}
