//! `ftParam*Stats`: provenance at the callback boundary, before hit logs
//! and status changes discard it. Campaign fighters queue events; VS and
//! Training retain hit provenance without allocating a campaign ledger.
use super::bonus::{DamageObject, DeadStatus, DefeatRecord, HitAttackId};
use crate::{
    combat::DamageBy,
    fighter::{Fighter, FighterKind},
    status::{AnyStatus, Status},
};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU16, Ordering};

static NEXT_COUNT: AtomicU16 = AtomicU16::new(1);

/// `ftManagerAllocFighter`: a new overlay starts the shared sequence at 1.
pub fn reset_count() {
    NEXT_COUNT.store(1, Ordering::Relaxed);
}

/// `ftParamGetStatUpdateCount`: one shared nonzero u16 sequence.
pub fn next_count() -> u16 {
    let mut current = NEXT_COUNT.load(Ordering::Relaxed);
    loop {
        match NEXT_COUNT.compare_exchange_weak(
            current,
            current.wrapping_add(1).max(1),
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return current,
            Err(actual) => current = actual,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags(pub u16);

impl Flags {
    pub fn id(self) -> HitAttackId {
        const IDS: &[HitAttackId] = &[
            HitAttackId::None,
            HitAttackId::Attack11,
            HitAttackId::AttackDash,
            HitAttackId::AttackS3,
            HitAttackId::AttackHi3,
            HitAttackId::AttackLw3,
            HitAttackId::AttackS4,
            HitAttackId::AttackHi4,
            HitAttackId::AttackLw4,
            HitAttackId::AttackAirN,
            HitAttackId::AttackAirF,
            HitAttackId::AttackAirB,
            HitAttackId::AttackAirHi,
            HitAttackId::AttackAirLw,
            HitAttackId::Attack12,
            HitAttackId::Attack13,
            HitAttackId::Attack100,
            HitAttackId::SpecialHi,
            HitAttackId::SpecialN,
            HitAttackId::SpecialNCopyMario,
            HitAttackId::SpecialNCopyLuigi,
            HitAttackId::SpecialNCopyFox,
            HitAttackId::SpecialNCopySamus,
            HitAttackId::SpecialNCopyDonkey,
            HitAttackId::SpecialNCopyPikachu,
            HitAttackId::SpecialNCopyNess,
            HitAttackId::SpecialNCopyLink,
            HitAttackId::SpecialNCopyPurin,
            HitAttackId::SpecialNCopyCaptain,
            HitAttackId::SpecialNCopyYoshi,
            HitAttackId::SpecialLw,
            HitAttackId::DownAttackD,
            HitAttackId::DownAttackU,
            HitAttackId::CliffAttackQuick,
            HitAttackId::CliffAttackSlow,
            HitAttackId::ThrowF,
            HitAttackId::ThrowB,
            HitAttackId::SwordSwing1,
            HitAttackId::SwordSwing3,
            HitAttackId::SwordSwing4,
            HitAttackId::SwordSwingDash,
            HitAttackId::BatSwing1,
            HitAttackId::BatSwing3,
            HitAttackId::BatSwing4,
            HitAttackId::BatSwingDash,
            HitAttackId::HarisenSwing1,
            HitAttackId::HarisenSwing3,
            HitAttackId::HarisenSwing4,
            HitAttackId::HarisenSwingDash,
            HitAttackId::StarRodSwing1,
            HitAttackId::StarRodSwing3,
            HitAttackId::StarRodSwing4,
            HitAttackId::StarRodSwingDash,
            HitAttackId::LGunShoot,
            HitAttackId::FireFlowerShoot,
            HitAttackId::Hammer,
            HitAttackId::ItemThrow,
            HitAttackId::Null,
        ];
        IDS[(self.0 & 0x3FF) as usize]
    }
    pub fn smash(self) -> bool {
        self.0 & 0x1000 != 0
    }
    pub fn air(self) -> bool {
        self.0 & 0x800 != 0
    }
    pub fn projectile(self) -> bool {
        self.0 & 0x400 != 0
    }
    pub fn body(self) -> Self {
        Self(self.0 & !0x400)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttackStat {
    pub flags: Flags,
    pub count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Attack(Flags),
    Defend { player: Option<u8>, flags: Flags },
    Credit { player: u8, damage: u32 },
    Damage(u32),
    Item(crate::item::utility::Kind),
    ShieldBreak { player: Option<u8> },
    Mew,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    pub attack: AttackStat,
    pub damage: AttackStat,
    pub damage_object: DamageObject,
    pub damage_count: u32,
    pending: Option<Vec<Event>>,
}

impl Stats {
    pub fn enable(&mut self) {
        self.pending = Some(Vec::new());
    }
    /// Rebirth resets FTStruct fields but preserves undrained callbacks
    /// and the host's campaign attachment, which live outside that struct.
    pub fn reinit(&mut self) -> Self {
        Self {
            pending: self.pending.take(),
            ..Self::default()
        }
    }
    pub fn emit(&mut self, event: Event) {
        if let Some(pending) = &mut self.pending {
            pending.push(event);
        }
    }
    pub fn drain(&mut self) -> impl Iterator<Item = Event> + '_ {
        self.pending.iter_mut().flat_map(|p| p.drain(..))
    }
    /// A proc_status or repeated-shot callback starts another attack with
    /// the same flags, independently of the stale-move count.
    pub fn restart(&mut self) {
        self.attack.count = next_count();
        self.count_attack(Flags(0));
    }
    fn count_attack(&mut self, previous: Flags) {
        if self.attack.flags.id() != HitAttackId::None && self.attack.flags.id() != previous.id() {
            self.emit(Event::Attack(self.attack.flags));
        }
    }
    pub fn defeat(&self, f: &Fighter, team_order: u8) -> DefeatRecord {
        DefeatRecord {
            team_order,
            status: match f.status.status {
                AnyStatus::Common(Status::DeadDown) => DeadStatus::Down,
                AnyStatus::Common(Status::DeadLeftRight) => DeadStatus::LeftRight,
                AnyStatus::Common(Status::DeadUpStar) => DeadStatus::UpStar,
                AnyStatus::Common(Status::DeadUpFall) => DeadStatus::UpFall,
                _ => DeadStatus::Other,
            },
            damage_player: f.damage_player,
            object: self.damage_object,
            attack_id: self.damage.flags.id(),
            stat_count: self.damage.count,
        }
    }
}

pub fn status_flags(kind: FighterKind, status: AnyStatus) -> Flags {
    status_flags_by_id(kind, status.id())
}

/// Authored descriptor flags, also used by the ROM equivalence check.
pub fn status_flags_by_id(kind: FighterKind, status_id: u16) -> Flags {
    use super::stat_flags as t;
    let id = usize::from(status_id);
    if id < 220 {
        return Flags(t::COMMON[id]);
    }
    let kind = kind.polygon_base().unwrap_or(match kind {
        FighterKind::MetalMario => FighterKind::Mario,
        FighterKind::GiantDonkey => FighterKind::Donkey,
        other => other,
    });
    let table: &[u16] = match kind {
        FighterKind::Mario => &t::MARIO,
        FighterKind::Fox => &t::FOX,
        FighterKind::Donkey => &t::DONKEY,
        FighterKind::Samus => &t::SAMUS,
        FighterKind::Luigi => &t::LUIGI,
        FighterKind::Link => &t::LINK,
        FighterKind::Yoshi => &t::YOSHI,
        FighterKind::Captain => &t::CAPTAIN,
        FighterKind::Kirby => &t::KIRBY,
        FighterKind::Pikachu => &t::PIKACHU,
        FighterKind::Purin => &t::PURIN,
        FighterKind::Ness => &t::NESS,
        _ => return Flags(0),
    };
    Flags(*table.get(id - 220).expect("ported special status"))
}

pub fn on_set_status(f: &mut Fighter, status: AnyStatus) {
    let previous = f.stats.attack.flags;
    let new = status_flags(f.kind, status);
    if new.id() == HitAttackId::None || new.id() != previous.id() {
        f.stats.attack = AttackStat {
            flags: new,
            count: next_count(),
        };
    }
    // These callbacks run inside ftMainSetStatus, before its final stat
    // update. A newly entered jab/low tilt can therefore count twice.
    if matches!(
        status,
        AnyStatus::Common(Status::Attack11 | Status::AttackLw3)
    ) {
        f.stats.restart();
    }
    f.stats.count_attack(previous);
}

/// `ftParamUpdate1PGameDamageStats`. Even a duplicate count changes
/// ownership/object and damage_count; only flags and defend counters dedup.
pub fn hit(f: &mut Fighter, by: DamageBy, stat: AttackStat, object: DamageObject) {
    match by {
        DamageBy::Player(p) => f.damage_player = Some(p),
        DamageBy::World => f.damage_player = None,
        DamageBy::Keep => {}
    }
    f.stats.damage_object = object;
    f.stats.damage_count += 1;
    let stat = if by == DamageBy::World {
        AttackStat::default()
    } else {
        stat
    };
    if stat.count == 0 || f.stats.damage.count != stat.count {
        f.stats.damage = stat;
        f.stats.emit(Event::Defend {
            player: f.damage_player,
            flags: stat.flags,
        });
    }
}
