//! `ittarget.c`: a stationary/animated item, destroyed by its first damage
//! callback. The scene consumes its objective event after the hit pass.
use super::*;

/// `dITTargetItemDesc`'s attributes: offset 0 of file 253
/// (`ITBonus1ObjectHeader`).
pub static ATTRIBUTES: ItemAttributes = super::attributes::TARGET;

pub(super) fn make(pos: Vec3, instance: u8) -> Item {
    let mut item = Item::new(
        ItemKind::Target,
        &ATTRIBUTES,
        ItemStatus::Target,
        AttackState::Off,
        pos,
        Vec3::ZERO,
        0,
    );
    item.ga = Ga::Ground;
    item.vars.target_index = instance;
    item.update_attack_positions();
    item
}

pub(super) fn hit(item: &Item, proc: HitProc, ctx: &mut HitCtx<'_>) -> Option<bool> {
    if proc != HitProc::Damage {
        return None;
    }
    ctx.fx
        .push(crate::wpeffect::WeaponEffect::ShieldBreak(item.pos));
    ctx.fx
        .push(crate::wpeffect::WeaponEffect::FireGrind(item.pos));
    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMBonus1TargetBreak);
    (ctx.events)(StageItemEvent::TargetBroken);
    Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fighter::FighterKind,
        stage::{StageItem, StageItems},
        weapon::{WeaponKind, WeaponPool, WeaponSpawn},
    };

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn weapons_break_all_ten_targets_once_through_the_real_hit_pass() {
        let mut pool = ItemPool::default();
        for i in 0..10 {
            pool.make_item(StageItem::Target(i), Vec3::ZERO).unwrap();
        }
        let f = Fighter::new(FighterKind::Mario, 0, 3);
        let mut weapons = WeaponPool::default();
        assert!(weapons.spawn(WeaponSpawn {
            kind: WeaponKind::MarioFireball,
            owner_port: 0,
            team: 0,
            position: Vec3::ZERO,
            facing: 1.0,
            stale: crate::stale::WeaponStale::of(&f)
        }));
        pool.search_hurt(&mut [], &mut weapons);
        pool.resolve(&[], &mut NoItemAnims, core::iter::empty);
        let count = pool
            .take_stage_events()
            .filter(|e| *e == StageItemEvent::TargetBroken)
            .count();
        assert_eq!(count, 10);
        assert_eq!(pool.active_count(), 0);
        pool.resolve(&[], &mut NoItemAnims, core::iter::empty);
        assert_eq!(pool.take_stage_events().count(), 0);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn targets_are_grounded_non_pickup_items_with_no_attack() {
        let mut item = make(Vec3::new(50.0, 100.0, 0.0), 4);
        assert_eq!(item.ga, Ga::Ground);
        assert!(item.floor.line.is_none());
        assert_eq!(item.attack.state, AttackState::Off);
        assert_eq!(item.damage_coll.hitstatus, HitStatus::Normal);
        assert!(!item.is_allow_pickup);
        assert_eq!(item.anim_target(), ItemAnimTarget::Target(4));
        let pos = item.pos;
        let mut pool = ItemPool::default();
        pool.alloc(item);
        pool.tick(core::iter::empty, None, &[], &mut NoItemAnims);
        item = *pool.items().next().unwrap();
        assert_eq!(item.pos, pos);
    }
}
