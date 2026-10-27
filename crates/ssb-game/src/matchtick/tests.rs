//! The order [`super::step`] runs the frame in, against the decomp's
//! process table (`gcRunAll`: priority 5 to 0, then link id, then link
//! order), plus the port-only settle (D-052).

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
        phase: |_| Phase::Interface,
        per_fighter: false,
        priority: 5,
        link: 10,
        decomp: "ifcommon.c:2309, 2256, 2046 entry, focus, countdown threads",
    },
    Process {
        phase: |_| Phase::StageController,
        per_fighter: false,
        priority: 4,
        link: 1,
        decomp: "grcastle.c:38 grCastleBumperProcUpdate, gr*ProcUpdate",
    },
    Process {
        phase: |_| Phase::Yakumono,
        per_fighter: false,
        priority: 4,
        link: 2,
        decomp: "grdisplay.c:209 mpCollisionPlayYakumonoAnim (GroundDisplay link)",
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
    Process {
        phase: Phase::Hits,
        per_fighter: true,
        priority: 1,
        link: 3,
        decomp: "ftmanager.c:862 ftMainProcSearchHitAll",
    },
    Process {
        phase: |_| Phase::ItemHits,
        per_fighter: false,
        priority: 1,
        link: 4,
        decomp: "itmanager.c:416 itProcessProcSearchHitAll",
    },
    Process {
        phase: |_| Phase::WeaponHits,
        per_fighter: false,
        priority: 1,
        link: 5,
        decomp: "wpmanager.c:305 wpProcessProcSearchHitWeapon",
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

/// [`decomp_order`] as the port runs it: the decomp's order, plus the
/// port's first-frame settle of the statuses the hit phase set, which is no
/// process of its own (D-052).
fn port_order(seats: u8, held: &[u8]) -> Vec<Phase> {
    let mut order = decomp_order(seats, held);
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
        go: false,
        rumble: None,
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
fn step_runs_the_decomp_process_order() {
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
        &order[..10],
        &[
            Phase::GroundAnim,
            Phase::Interrupt(0),
            Phase::Interrupt(1),
            Phase::Interface,
            Phase::StageController,
            Phase::Yakumono,
            Phase::Physics(0),
            Phase::ItemActor,
            Phase::Capture(1),
            Phase::Items,
        ]
    );
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

/// A seat that notes the countdown lock each pass sees.
struct LockProbe {
    fighter: Fighter,
    at_interrupt: Option<bool>,
    at_physics: Option<bool>,
}

impl HasFighter for LockProbe {
    fn fighter(&self) -> &Fighter {
        &self.fighter
    }
    fn fighter_mut(&mut self) -> &mut Fighter {
        &mut self.fighter
    }
}

impl Seat<NoGround> for LockProbe {
    fn interrupt(&mut self, _: &NoGround, _: &InputView<'_>) {
        self.at_interrupt = Some(self.fighter.interface.control_disable);
    }
    fn physics(&mut self, _: &NoGround) {
        self.at_physics = Some(self.fighter.interface.control_disable);
    }
    fn settle_catch(&mut self, _: &NoGround) {}
    fn settle_capture(&mut self, _: &NoGround) {}
    fn settle_status(&mut self, _: &NoGround) {}
}

/// "Go" (`ifCommonAnnounceGoSetStatus`, link 11, priority 5) runs after
/// the fighters' priority-5 interrupt, so on its frame the interrupt still
/// reads the lock and physics (priority 4) does not: the RE-556 oracle's
/// frame 390, with the first CPU draw at 391.
#[test]
fn go_unlocks_after_the_interrupt_and_before_physics() {
    let mut probe = LockProbe {
        fighter: Fighter::new(FighterKind::Mario, 0, 3),
        at_interrupt: None,
        at_physics: None,
    };
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
        locked: true,
        go: true,
        rumble: None,
    };
    let mut seats: Seats<'_, LockProbe> = [Some(&mut probe), None, None, None];
    step(
        &mut Bare(NoGround, NoProps),
        &mut seats,
        &mut world,
        &mut (),
    );
    assert!(!world.locked && !world.go);
    assert_eq!(probe.at_interrupt, Some(true));
    assert_eq!(probe.at_physics, Some(false));
}

/// One battle, one pair of counts (`ftmanager.c:163-164`): the seats and
/// pools draw from the one sequence the item pool's setup started, and a
/// frame neither restarts it nor repeats a count any of them drew.
#[test]
fn step_keeps_one_sequence_for_every_handle() {
    let mut weapons: Box<WeaponPool> = Box::default();
    let mut items: Box<ItemPool> = Box::default();
    let mut fighters: Vec<Fighter> = (0..2)
        .map(|i| {
            let mut f = Fighter::new(FighterKind::Mario, i, 3);
            f.pos.x = f32::from(i) * 1000.0;
            f
        })
        .collect();
    // Draws through seat 0 (3), seat 1 (5) and the weapons (2).
    let first = items.counts.peek().0;
    let mut drawn = Vec::new();
    for (handle, n) in [
        (&fighters[0].stats.counts, 3),
        (&fighters[1].stats.counts, 5),
        (&weapons.counts, 2),
    ] {
        for _ in 0..n {
            drawn.push(handle.motion());
        }
    }
    let expected: Vec<u16> = (first..first + 10).collect();
    assert_eq!(drawn, expected);
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
        go: false,
        rumble: None,
    };
    let mut it = fighters.iter_mut();
    let mut seats: Seats<'_, Fighter> = [it.next(), it.next(), None, None];
    step(
        &mut Bare(NoGround, NoProps),
        &mut seats,
        &mut world,
        &mut (),
    );
    assert!(world.items.counts.motion() > first + 9);
}

struct CameraSeat {
    fighter: Fighter,
    rig: Option<CameraRig>,
    framing: Framing,
}

impl HasFighter for CameraSeat {
    fn fighter(&self) -> &Fighter {
        &self.fighter
    }
    fn fighter_mut(&mut self) -> &mut Fighter {
        &mut self.fighter
    }
    fn rig(&self) -> Option<&CameraRig> {
        self.rig.as_ref()
    }
    fn rig_mut(&mut self) -> Option<&mut CameraRig> {
        self.rig.as_mut()
    }
    fn framing(&self) -> Framing {
        self.framing
    }
}

/// The camera pass frames the lead first, then the others in link order,
/// and resolves the demo and entry focuses on their fighters as this tick
/// left them, all through the lead's rig.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_camera_pass_frames_every_seat_through_the_leads_rig() {
    use crate::camera::{interest_of, BonusFollow, Bounds, DemoZoom, EntryZoom, StageCamera};
    use ssb_engine::math::Vec3;
    let half = |h: f32| Bounds {
        top: h,
        bottom: -h,
        left: -h,
        right: h,
    };
    let stage = CameraStage {
        camera: half(4000.0),
        team_camera: half(2000.0),
        map_top: 6000.0,
        light_angle_z: 0.1,
        stage_camera: StageCamera::Zebes,
        follow: BonusFollow::Other,
    };
    let seat = |port: u8, x: f32, off: f32| {
        let mut fighter = Fighter::new(FighterKind::Mario, port, 3);
        fighter.pos = Vec3::new(x, 100.0, 0.0);
        CameraSeat {
            fighter,
            rig: None,
            framing: Framing {
                cam_offset_y: off,
                zoom_frame: 1.0,
            },
        }
    };
    let demo = DemoZoom {
        port: 2,
        eye: (0.1, -0.1),
        dist: 1500.0,
        pan_scale: 0.3,
        fov: 30.0,
        pos: Vec3::ZERO,
        cam_offset_y: 0.0,
    };
    for rig in [
        CameraRig::default(),
        CameraRig {
            demo_zoom: Some(demo),
            ..CameraRig::default()
        },
        CameraRig {
            entry_zoom: Some(EntryZoom {
                slot: 1,
                dist: 1200.0,
            }),
            ..CameraRig::default()
        },
    ] {
        let mut lead = seat(0, -600.0, 40.0);
        lead.rig = Some(rig);
        let (mut a, mut b) = (seat(2, 300.0, 70.0), seat(1, 900.0, 55.0));
        let mut expect = rig;
        if let Some(z) = expect.demo_zoom.as_mut() {
            (z.pos, z.cam_offset_y) = (a.fighter.pos, 70.0);
        }
        let entry = rig.entry_zoom.map(|z| EntryFocusTarget {
            pos: a.fighter.pos,
            cam_offset_y: 70.0,
            dist: z.dist,
        });
        let list: Vec<Interest> = [&lead, &a, &b]
            .iter()
            .filter_map(|s| interest_of(&s.fighter, s.framing, &stage))
            .collect();
        expect.tick(lead.fighter.pos, 40.0, &list, entry, &stage);
        let mut seats: Seats<'_, CameraSeat> = [Some(&mut lead), Some(&mut a), None, Some(&mut b)];
        camera_pass(&mut seats, &stage);
        assert_eq!(lead.rig, Some(expect));
        assert_ne!(expect.camera, rig.camera);
    }
}

/// A seat that logs its passes, by the serial of the fighter in it.
struct Member {
    fighter: Fighter,
    serial: u8,
    log: alloc::rc::Rc<core::cell::RefCell<Vec<(u8, char)>>>,
}

impl HasFighter for Member {
    fn fighter(&self) -> &Fighter {
        &self.fighter
    }
    fn fighter_mut(&mut self) -> &mut Fighter {
        &mut self.fighter
    }
}

impl Seat<NoGround> for Member {
    fn interrupt(&mut self, _: &NoGround, _: &InputView<'_>) {
        self.log.borrow_mut().push((self.serial, 'i'));
    }
    fn physics(&mut self, _: &NoGround) {
        self.log.borrow_mut().push((self.serial, 'p'));
    }
    fn settle_catch(&mut self, _: &NoGround) {}
    fn settle_capture(&mut self, _: &NoGround) {}
    fn settle_status(&mut self, _: &NoGround) {}
}

/// `sc1PGameSpawnEnemyTeamNext` as a host does it: the next member in the
/// seat, serials from 4.
struct NextMember(u8);

impl MatchHooks<Member> for NextMember {
    fn next_member(
        &mut self,
        seat: &mut Member,
        _: &[MapGroup],
        _: Option<&mut crate::battle::Battle>,
    ) -> bool {
        seat.fighter = Fighter::new(FighterKind::Mario, seat.fighter.port, 3);
        seat.serial = self.0;
        self.0 += 1;
        true
    }
}

/// The N64 Polygon stage (RE-458): a member made in an enemy's interrupt
/// runs its whole first frame in that frame, after the other fighters,
/// and stays last in the link (fighter link `L0132` after port 2's
/// replacement at t929). Two members replaced in one frame link in the
/// order their predecessors ran.
#[test]
fn a_next_member_runs_its_first_frame_last_in_the_link() {
    let log = alloc::rc::Rc::new(core::cell::RefCell::new(Vec::new()));
    let mut members: Vec<Member> = (0..4u8)
        .map(|i| Member {
            fighter: Fighter::new(FighterKind::Mario, i, 3),
            serial: i,
            log: log.clone(),
        })
        .collect();
    members[1].fighter.dead.enemy_next_pending = true;
    members[2].fighter.dead.enemy_next_pending = true;
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
        locked: true,
        go: false,
        rumble: None,
    };
    let mut it = members.iter_mut();
    let mut seats: Seats<'_, Member> = [it.next(), it.next(), it.next(), it.next()];
    let mut hooks = NextMember(4);
    step(
        &mut Bare(NoGround, NoProps),
        &mut seats,
        &mut world,
        &mut hooks,
    );
    let pass = |c: char| -> Vec<u8> {
        log.borrow()
            .iter()
            .filter(|e| e.1 == c)
            .map(|e| e.0)
            .collect()
    };
    assert_eq!(pass('i'), [0, 1, 2, 3, 4, 5]);
    assert_eq!(pass('p'), [0, 3, 4, 5]);
    let ports: Vec<u8> = seats.iter().flatten().map(|s| s.fighter.port).collect();
    assert_eq!(ports, [0, 3, 1, 2]);
    assert!(seats[3].as_ref().unwrap().fighter.interface.control_disable);
}
