use super::*;
use crate::combat::{self, AttackColl, AttackState};
use crate::fighter::{Fighter, FighterKind, Situation};
use crate::weapon::{WeaponKind, WeaponPool, WeaponSpawn};
use ssb_engine::math::{Vec2, Vec3};

const TEAM_ATTACK: TeamRules = TeamRules {
    is_team_battle: true,
    is_team_attack: true,
};

fn standing(kind: FighterKind, port: u8, team: u8) -> Fighter {
    let mut f = Fighter::new(kind, port, 3);
    f.team = team;
    crate::status::set_wait(&mut f);
    f.situation = Situation::Ground;
    f.floor = Some(crate::ground::Standing {
        line: 0,
        flags: 0,
        normal: Vec2::new(0.0, 1.0),
    });
    f
}

/// A 10-damage attack around the attacker's middle.
fn swing(f: &mut Fighter) {
    f.attack_colls[0] = AttackColl {
        state: AttackState::New,
        damage: 10,
        size: 150.0,
        angle: 361,
        kb_scale: 100,
        kb_base: 10,
        is_hit_air: true,
        is_hit_ground: true,
        offset: Vec3::new(0.0, 150.0, 0.0),
        ..Default::default()
    };
}

/// One frame of `ftMainSearchHitFighter` with `attacker` swinging at a
/// `victim` in its reach.
fn fighter_lands(rules: TeamRules, attacker_team: u8, victim_team: u8) -> bool {
    let mut attacker = standing(FighterKind::Mario, 0, attacker_team);
    let mut victim = standing(FighterKind::Fox, 1, victim_team);
    swing(&mut attacker);
    combat::search_all(&mut [&mut attacker, &mut victim], rules);
    combat::finish_frame(&mut [&mut attacker, &mut victim]);
    victim.damage > 0
}

#[test]
fn a_teammates_attack_passes_through_only_with_team_attack_off() {
    assert!(!fighter_lands(TeamRules::TEAMS, 0, 0));
    assert!(fighter_lands(TeamRules::TEAMS, 0, 1));
    assert!(fighter_lands(TEAM_ATTACK, 0, 0));
    assert!(fighter_lands(TeamRules::FREE_FOR_ALL, 0, 0));
}

/// A star Kirby (port 0, `thrower_team`) spat from a fighter of team 1
/// against a bystander of `bystander_team`.
fn star_lands(rules: TeamRules, thrower_team: u8, bystander_team: u8) -> bool {
    let mut star = Fighter::new(FighterKind::Mario, 1, 3);
    star.team = 1;
    crate::capture_kirby::set_star(&mut star, false, Vec3::new(40.0, 0.0, 0.0), 0, thrower_team);
    let mut bystander = standing(FighterKind::Fox, 2, bystander_team);
    bystander.pos = star.pos;
    combat::search_all(&mut [&mut star, &mut bystander], rules);
    combat::finish_frame(&mut [&mut star, &mut bystander]);
    bystander.damage > 0
}

/// `(other_fp->throw_gobj != NULL) ? other_fp->throw_team : other_fp->team`:
/// the star counts for its thrower's team, not its own.
#[test]
fn a_thrown_star_counts_for_its_throwers_team() {
    // The thrower's teammate is spared, though the star's own team is 1.
    assert!(!star_lands(TeamRules::TEAMS, 0, 0));
    // The star's own teammate is not.
    assert!(star_lands(TeamRules::TEAMS, 0, 1));
    assert!(star_lands(TEAM_ATTACK, 0, 0));
    assert!(star_lands(TeamRules::FREE_FOR_ALL, 0, 0));
}

fn fireball(weapons: &mut WeaponPool, owner_port: u8, team: u8, facing: f32) {
    assert!(weapons.spawn(WeaponSpawn {
        kind: WeaponKind::MarioFireball,
        owner_port,
        team,
        stale: crate::stale::WeaponStale::FRESH,
        position: Vec3::new(0.0, 150.0, 0.0),
        facing,
    }));
}

/// `ftMainSearchHitWeapon`: Mario's (team 0) Fireball against a Fox of
/// `victim_team`.
fn weapon_lands(rules: TeamRules, victim_team: u8) -> bool {
    let mut weapons = WeaponPool::default();
    weapons.team_rules = rules;
    fireball(&mut weapons, 0, 0, 1.0);
    let mut victim = standing(FighterKind::Fox, 1, victim_team);
    weapons.apply_hits(&mut victim);
    combat::resolve(&mut victim);
    victim.damage > 0
}

#[test]
fn a_teammates_weapon_passes_through_only_with_team_attack_off() {
    assert!(!weapon_lands(TeamRules::TEAMS, 0));
    assert!(weapon_lands(TeamRules::TEAMS, 1));
    assert!(weapon_lands(TEAM_ATTACK, 0));
    assert!(weapon_lands(TeamRules::FREE_FOR_ALL, 0));
}

/// `wpProcessProcHitCollisions`: a reflected weapon takes its reflector's
/// team, so it then spares the reflector's teammates and hits the thrower's.
#[test]
fn a_reflected_weapon_takes_the_reflectors_team() {
    let mut weapons = WeaponPool::default();
    weapons.team_rules = TeamRules::TEAMS;
    // Mario's Fireball flies left into Fox's reflector.
    assert!(weapons.spawn(WeaponSpawn {
        kind: WeaponKind::MarioFireball,
        owner_port: 0,
        team: 0,
        stale: crate::stale::WeaponStale::FRESH,
        position: Vec3::new(100.0, 60.0, 0.0),
        facing: -1.0,
    }));
    let mut fox = standing(FighterKind::Fox, 1, 1);
    crate::status::set_fox_special_lw_start(&mut fox);
    crate::status::set_any_status(
        &mut fox,
        crate::status::AnyStatus::Fox(crate::status::FoxStatus::SpecialLwLoop),
        0.0,
        crate::status::StatusTiming::unknown(),
    );
    weapons.apply_hits(&mut fox);
    combat::resolve(&mut fox);
    assert_eq!(weapons.first_fireball().map(|f| f.owner_port), Some(1));
    let mut fox_mate = standing(FighterKind::Fox, 2, 1);
    weapons.apply_hits(&mut fox_mate);
    combat::resolve(&mut fox_mate);
    assert_eq!(fox_mate.damage, 0);
    let mut mario_mate = standing(FighterKind::Fox, 3, 0);
    weapons.apply_hits(&mut mario_mate);
    combat::resolve(&mut mario_mate);
    assert!(mario_mate.damage > 0);
}
