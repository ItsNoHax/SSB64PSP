//! Save data — `lb/lbbackup.c`, `LBBackupData` (`lb/lbtypes.h`) and
//! `dSCManagerDefaultBackupData` (`sc/scmanager.c`).
//!
//! [`Backup`] is `gSCManagerBackupData`: the VS records, the options, the
//! unlock and fighter masks, the 1P records and the counters the unlocks
//! read. The N64 keeps two copies of it in SRAM, at 0 and at the struct's
//! size rounded up to 16; [`Backup::image`] lays out the same 0xBDC bytes
//! (big-endian, with the struct's padding zeroed), and [`load`] is
//! `lbBackupIsSramValid`'s fallback from the first copy to the second to
//! the defaults. The host persists the image whenever [`Backup::writes`]
//! moves (`lbBackupWrite`).
//!
//! [`Backup::apply_options`] is `lbBackupApplyOptions`: it sets the audio
//! quality; the screen centre offsets have nothing to apply on the PSP
//! display and are only kept and saved.

use crate::fighter::FighterKind;
use crate::spgame::{Difficulty, Unlock};

/// `sizeof(LBBackupData)`: `dSCManagerDefaultBackupData` (0x800A3994) ends
/// where `dSCManagerDefaultSceneData` (0x800A3F80) starts.
pub const SIZE: usize = 0x5EC;
/// `ALIGN(sizeof(LBBackupData), 0x10)`: the second SRAM copy.
pub const COPY_OFFSET: usize = 0x5F0;
/// Both copies, as `lbBackupWrite` leaves SRAM.
pub const IMAGE_SIZE: usize = COPY_OFFSET + SIZE;
/// `signature`'s expected value; only the defaults ever hold it.
pub const SIGNATURE: u16 = 0x29A;

/// `LBBACKUP_CHARACTER_MASK_ALL`.
pub const CHARACTER_MASK_ALL: u16 = 0x0FFF;
/// `LBBACKUP_CHARACTER_MASK_UNLOCK`: Luigi, Captain Falcon, Jigglypuff and
/// Ness.
pub const CHARACTER_MASK_UNLOCK: u16 = (1 << FighterKind::Luigi as u16)
    | (1 << FighterKind::Captain as u16)
    | (1 << FighterKind::Purin as u16)
    | (1 << FighterKind::Ness as u16);
/// `LBBACKUP_CHARACTER_MASK_STARTER`.
pub const CHARACTER_MASK_STARTER: u16 = CHARACTER_MASK_ALL & !CHARACTER_MASK_UNLOCK;
/// `LBBACKUP_GROUND_MASK_ALL`: the eight starter VS stages.
pub const GROUND_MASK_ALL: u16 = 0x00FF;

/// `LBBACKUP_UNLOCK_MASK_ALL`.
pub const UNLOCK_MASK_ALL: u8 = 0x7F;
/// `LBBACKUP_UNLOCK_MASK_NEWCOMERS`.
pub const UNLOCK_MASK_NEWCOMERS: u8 = (1 << Unlock::Luigi as u8)
    | (1 << Unlock::Ness as u8)
    | (1 << Unlock::Captain as u8)
    | (1 << Unlock::Purin as u8);
/// `LBBACKUP_UNLOCK_MASK_PRIZE`: Mushroom Kingdom, Sound Test, Item Switch.
pub const UNLOCK_MASK_PRIZE: u8 = UNLOCK_MASK_ALL & !UNLOCK_MASK_NEWCOMERS;

/// `LBBACKUP_ERROR_RANDOMKNOCKBACK`.
pub const ERROR_RANDOM_KNOCKBACK: u8 = 1 << 0;
/// `LBBACKUP_ERROR_HALFSTICKRANGE`.
pub const ERROR_HALF_STICK_RANGE: u8 = 1 << 1;
/// `LBBACKUP_ERROR_1PGAMEMARIO`.
pub const ERROR_1PGAME_MARIO: u8 = 1 << 2;
/// `LBBACKUP_ERROR_VSBATTLECASTLE`.
pub const ERROR_VS_BATTLE_CASTLE: u8 = 1 << 3;

/// `I_MIN_TO_TICS(60)`: an unbeaten bonus stage's time.
pub const BONUS_TIME_DEFAULT: u32 = 60 * 60 * 60;

/// One `LBBackupVSRecord`, indexed by the fighter it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VsRecord {
    /// KOs scored on each fighter.
    pub ko_count: [u16; 12],
    pub time_used: u32,
    pub damage_given: u32,
    pub damage_taken: u32,
    pub unk: u16,
    pub selfdestructs: u16,
    pub games_played: u16,
    pub player_count_tally: u16,
    pub player_count_tallies: [u16; 12],
    pub played_against: [u16; 12],
}

/// One `LBBackup1PRecord`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    pub spgame_hiscore: u32,
    pub spgame_continues: u32,
    pub spgame_total_bonuses: u32,
    pub spgame_best_difficulty: u8,
    pub bonus1_task_count: u8,
    pub bonus2_task_count: u8,
    pub bonus1_time: u32,
    pub bonus2_time: u32,
    pub is_spgame_complete: bool,
}

impl Default for Record {
    fn default() -> Self {
        Self {
            spgame_hiscore: 0,
            spgame_continues: 0,
            spgame_total_bonuses: 0,
            spgame_best_difficulty: 0,
            bonus1_task_count: 0,
            bonus2_task_count: 0,
            bonus1_time: BONUS_TIME_DEFAULT,
            bonus2_time: BONUS_TIME_DEFAULT,
            is_spgame_complete: false,
        }
    }
}

/// `gSCManagerBackupData` (`LBBackupData`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    pub vs_records: [VsRecord; 12],
    pub is_allow_screenflash: bool,
    /// 0 mono, 1 stereo.
    pub sound_mono_or_stereo: u8,
    pub screen_adjust_h: i16,
    pub screen_adjust_v: i16,
    /// `characters_fkind`: the Characters menu's last fighter, which
    /// `mnMessageApplyUnlock` points at a newcomer.
    pub characters_fkind: FighterKind,
    pub unlock_mask: u8,
    /// Unlocked newcomers; the selects add [`CHARACTER_MASK_STARTER`].
    pub fighter_mask: u16,
    pub spgame_difficulty: Difficulty,
    /// Stocks chosen on the 1P select, 0..=4.
    pub spgame_stock_count: i8,
    pub spgame_records: [Record; 12],
    /// VS stages played.
    pub ground_mask: u16,
    pub vs_itemswitch_battles: u8,
    pub vs_total_battles: u16,
    pub error_flags: u8,
    /// Boots that reached the title screen (`mnTitleStartScene`).
    pub boot: u8,
    pub signature: u16,
    /// `lbBackupWrite` calls, for the host to persist. Not saved.
    pub writes: u32,
}

impl Default for Backup {
    /// `dSCManagerDefaultBackupData`.
    fn default() -> Self {
        Self {
            vs_records: [VsRecord::default(); 12],
            is_allow_screenflash: true,
            sound_mono_or_stereo: 1,
            screen_adjust_h: 0,
            screen_adjust_v: 0,
            characters_fkind: FighterKind::Mario,
            unlock_mask: 0,
            fighter_mask: 0,
            spgame_difficulty: Difficulty::Easy,
            spgame_stock_count: 2,
            spgame_records: [Record::default(); 12],
            ground_mask: 0,
            vs_itemswitch_battles: 0,
            vs_total_battles: 0,
            error_flags: 0,
            boot: 0,
            signature: SIGNATURE,
            writes: 0,
        }
    }
}

/// Big-endian field writer over one copy.
struct Out<'a> {
    b: &'a mut [u8],
    at: usize,
}

impl Out<'_> {
    fn u8(&mut self, v: u8) {
        self.b[self.at] = v;
        self.at += 1;
    }
    fn u16(&mut self, v: u16) {
        self.at = self.at.next_multiple_of(2);
        self.b[self.at..self.at + 2].copy_from_slice(&v.to_be_bytes());
        self.at += 2;
    }
    fn u32(&mut self, v: u32) {
        self.at = self.at.next_multiple_of(4);
        self.b[self.at..self.at + 4].copy_from_slice(&v.to_be_bytes());
        self.at += 4;
    }
}

/// Big-endian field reader, with the same alignment as [`Out`].
struct In<'a> {
    b: &'a [u8],
    at: usize,
}

impl In<'_> {
    fn u8(&mut self) -> u8 {
        let v = self.b[self.at];
        self.at += 1;
        v
    }
    fn u16(&mut self) -> u16 {
        self.at = self.at.next_multiple_of(2);
        let v = u16::from_be_bytes([self.b[self.at], self.b[self.at + 1]]);
        self.at += 2;
        v
    }
    fn u32(&mut self) -> u32 {
        self.at = self.at.next_multiple_of(4);
        let mut w = [0; 4];
        w.copy_from_slice(&self.b[self.at..self.at + 4]);
        self.at += 4;
        u32::from_be_bytes(w)
    }
}

fn difficulty(v: u8) -> Option<Difficulty> {
    Some(match v {
        0 => Difficulty::VeryEasy,
        1 => Difficulty::Easy,
        2 => Difficulty::Normal,
        3 => Difficulty::Hard,
        4 => Difficulty::VeryHard,
        _ => return None,
    })
}

/// `lbBackupCreateChecksum`: every byte before `checksum`, weighted by its
/// position from 1.
pub fn checksum(copy: &[u8]) -> i32 {
    copy[..SIZE - 4]
        .iter()
        .enumerate()
        .fold(0i32, |sum, (i, &b)| {
            sum.wrapping_add(i32::from(b).wrapping_mul(i as i32 + 1))
        })
}

/// What [`load`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loaded {
    /// The first copy was valid; nothing is written.
    First,
    /// The first copy was not and the second was: both are rewritten.
    Second,
    /// Neither was: the defaults are written (`lbBackupIsSramValid` returns
    /// FALSE).
    Defaults,
}

/// `lbBackupIsSramValid` over an SRAM image (`None` or short for a missing
/// save, which reads as invalid). A write the source makes here is counted
/// in [`Backup::writes`].
pub fn load(image: Option<&[u8]>) -> (Backup, Loaded) {
    let copy = |at: usize| {
        image
            .and_then(|i| i.get(at..at + SIZE))
            .and_then(Backup::decode)
    };
    if let Some(b) = copy(0) {
        return (b, Loaded::First);
    }
    if let Some(mut b) = copy(COPY_OFFSET) {
        b.write();
        return (b, Loaded::Second);
    }
    let mut b = Backup::default();
    b.write();
    (b, Loaded::Defaults)
}

impl Backup {
    /// `lbBackupWrite`: the host saves [`Self::image`] when the count moves.
    pub fn write(&mut self) {
        self.writes = self.writes.wrapping_add(1);
    }

    /// `lbBackupApplyOptions`: `syAudioSetQuality` with the saved mono or
    /// stereo setting. `syVideoSetCenterOffsets` has no PSP counterpart.
    pub fn apply_options(&self) {
        crate::sound::set_quality(u32::from(self.sound_mono_or_stereo));
    }

    /// One copy as the N64 lays out `LBBackupData`, with its checksum.
    pub fn encode(&self) -> [u8; SIZE] {
        let mut b = [0u8; SIZE];
        let mut o = Out { b: &mut b, at: 0 };
        for r in &self.vs_records {
            for &k in &r.ko_count {
                o.u16(k);
            }
            o.u32(r.time_used);
            o.u32(r.damage_given);
            o.u32(r.damage_taken);
            o.u16(r.unk);
            o.u16(r.selfdestructs);
            o.u16(r.games_played);
            o.u16(r.player_count_tally);
            for &t in &r.player_count_tallies {
                o.u16(t);
            }
            for &p in &r.played_against {
                o.u16(p);
            }
            o.at = o.at.next_multiple_of(4);
        }
        o.u8(u8::from(self.is_allow_screenflash));
        o.u8(self.sound_mono_or_stereo);
        o.u16(self.screen_adjust_h as u16);
        o.u16(self.screen_adjust_v as u16);
        o.u8(self.characters_fkind as u8);
        o.u8(self.unlock_mask);
        o.u16(self.fighter_mask);
        o.u8(self.spgame_difficulty as u8);
        o.u8(self.spgame_stock_count as u8);
        for r in &self.spgame_records {
            o.u32(r.spgame_hiscore);
            o.u32(r.spgame_continues);
            o.u32(r.spgame_total_bonuses);
            o.u8(r.spgame_best_difficulty);
            o.u32(r.bonus1_time);
            o.u8(r.bonus1_task_count);
            o.u32(r.bonus2_time);
            o.u8(r.bonus2_task_count);
            o.u8(u8::from(r.is_spgame_complete));
            o.at = o.at.next_multiple_of(4);
        }
        o.u16(self.ground_mask);
        o.u8(self.vs_itemswitch_battles);
        o.u16(self.vs_total_battles);
        o.u8(self.error_flags);
        o.u8(self.boot);
        o.u16(self.signature);
        debug_assert_eq!(o.at.next_multiple_of(4), SIZE - 4);
        let sum = checksum(&b);
        b[SIZE - 4..].copy_from_slice(&sum.to_be_bytes());
        b
    }

    /// `lbBackupIsChecksumValid` over one copy, then its fields. A copy
    /// whose checksum holds but whose last character or difficulty is not
    /// one the game has is also refused (the N64 would read it raw).
    pub fn decode(copy: &[u8]) -> Option<Backup> {
        let copy = copy.get(..SIZE)?;
        let mut sum = [0; 4];
        sum.copy_from_slice(&copy[SIZE - 4..]);
        if checksum(copy) != i32::from_be_bytes(sum) {
            return None;
        }
        let mut i = In { b: copy, at: 0 };
        let mut b = Backup::default();
        for r in &mut b.vs_records {
            for k in &mut r.ko_count {
                *k = i.u16();
            }
            r.time_used = i.u32();
            r.damage_given = i.u32();
            r.damage_taken = i.u32();
            r.unk = i.u16();
            r.selfdestructs = i.u16();
            r.games_played = i.u16();
            r.player_count_tally = i.u16();
            for t in &mut r.player_count_tallies {
                *t = i.u16();
            }
            for p in &mut r.played_against {
                *p = i.u16();
            }
            i.at = i.at.next_multiple_of(4);
        }
        b.is_allow_screenflash = i.u8() != 0;
        b.sound_mono_or_stereo = i.u8();
        b.screen_adjust_h = i.u16() as i16;
        b.screen_adjust_v = i.u16() as i16;
        b.characters_fkind = FighterKind::from_ordinal(i.u8()).filter(|k| (*k as u8) < 12)?;
        b.unlock_mask = i.u8();
        b.fighter_mask = i.u16();
        b.spgame_difficulty = difficulty(i.u8())?;
        b.spgame_stock_count = i.u8() as i8;
        for r in &mut b.spgame_records {
            r.spgame_hiscore = i.u32();
            r.spgame_continues = i.u32();
            r.spgame_total_bonuses = i.u32();
            r.spgame_best_difficulty = i.u8();
            r.bonus1_time = i.u32();
            r.bonus1_task_count = i.u8();
            r.bonus2_time = i.u32();
            r.bonus2_task_count = i.u8();
            r.is_spgame_complete = i.u8() != 0;
            i.at = i.at.next_multiple_of(4);
        }
        b.ground_mask = i.u16();
        b.vs_itemswitch_battles = i.u8();
        b.vs_total_battles = i.u16();
        b.error_flags = i.u8();
        b.boot = i.u8();
        b.signature = i.u16();
        (b.signature == SIGNATURE).then_some(b)
    }

    /// SRAM after `lbBackupWrite`: the copy at 0 and again at
    /// [`COPY_OFFSET`].
    pub fn image(&self) -> [u8; IMAGE_SIZE] {
        let copy = self.encode();
        let mut image = [0u8; IMAGE_SIZE];
        image[..SIZE].copy_from_slice(&copy);
        image[COPY_OFFSET..].copy_from_slice(&copy);
        image
    }

    /// `fighter_mask | LBBACKUP_CHARACTER_MASK_STARTER`.
    pub fn unlocked_fighters(&self) -> u16 {
        self.fighter_mask | CHARACTER_MASK_STARTER
    }

    /// `lbBackupClearNewcomers`.
    pub fn clear_newcomers(&mut self) {
        let d = Backup::default();
        self.unlock_mask &= !UNLOCK_MASK_NEWCOMERS;
        self.unlock_mask |= d.unlock_mask;
        self.fighter_mask = d.fighter_mask;
    }

    /// `lbBackupClear1PHighScore`.
    pub fn clear_1p_high_score(&mut self) {
        let d = Record::default();
        for r in &mut self.spgame_records {
            r.spgame_hiscore = d.spgame_hiscore;
            r.spgame_continues = d.spgame_continues;
            r.spgame_total_bonuses = d.spgame_total_bonuses;
            r.spgame_best_difficulty = d.spgame_best_difficulty;
            r.is_spgame_complete = d.is_spgame_complete;
        }
    }

    /// `lbBackupClearVSRecord`.
    pub fn clear_vs_record(&mut self) {
        self.vs_records = [VsRecord::default(); 12];
        self.vs_total_battles = Backup::default().vs_total_battles;
    }

    /// `lbBackupClearBonusStageTime`.
    pub fn clear_bonus_stage_time(&mut self) {
        let d = Record::default();
        for r in &mut self.spgame_records {
            r.bonus1_time = d.bonus1_time;
            r.bonus1_task_count = d.bonus1_task_count;
            r.bonus2_time = d.bonus2_time;
            r.bonus2_task_count = d.bonus2_task_count;
        }
    }

    /// `lbBackupClearPrize`.
    pub fn clear_prize(&mut self) {
        let d = Backup::default();
        self.unlock_mask &= !UNLOCK_MASK_PRIZE;
        self.unlock_mask |= d.unlock_mask;
        self.ground_mask = d.ground_mask;
        self.vs_itemswitch_battles = d.vs_itemswitch_battles;
    }

    /// `lbBackupClearAllData`. The write count is the host's and carries.
    pub fn clear_all_data(&mut self) {
        *self = Backup {
            writes: self.writes,
            ..Backup::default()
        };
    }

    /// `mnTitleStartScene`'s boot count: once per power-on, before the
    /// title animation has been seen. `boot <= U8_MAX` always holds, so the
    /// count wraps.
    pub fn count_boot(&mut self) {
        self.boot = self.boot.wrapping_add(1);
        self.write();
    }

    /// `lbBackupCorrectErrors`' selections against the unlocked fighters
    /// and stages.
    pub fn correct_errors(&mut self, s: &mut Selections) {
        let unlocked = self.unlocked_fighters();
        let ok = |k: FighterKind| (k as u32) < 16 && unlocked & (1 << k as u16) != 0;
        if !ok(self.characters_fkind) {
            self.characters_fkind = Backup::default().characters_fkind;
        }
        for k in [
            &mut s.fkind,
            &mut s.training_man_fkind,
            &mut s.training_com_fkind,
        ] {
            if k.is_some_and(|k| !ok(k)) {
                *k = None;
            }
        }
        for p in &mut s.players {
            if p.fkind.is_some_and(|k| !ok(k)) {
                p.fkind = None;
                p.is_man = true;
            }
        }
        if self.unlock_mask & Unlock::Inishie.mask() == 0 {
            use crate::stage_select::gkind::{CASTLE, INISHIE};
            // `dSCManagerDefaultSceneData`'s stage-select kinds.
            for g in [&mut s.maps_vsmode_gkind, &mut s.maps_training_gkind] {
                if *g == INISHIE {
                    *g = CASTLE;
                }
            }
        }
        if self.unlock_mask & Unlock::ItemSwitch.mask() == 0 {
            s.items_reset = true;
        }
    }
}

/// One `gSCManagerTransferBattleState.players` slot for
/// [`Backup::correct_errors`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectedPlayer {
    /// `None` is `nFTKindNull`.
    pub fkind: Option<FighterKind>,
    /// `pkind == nFTPlayerKindMan`.
    pub is_man: bool,
}

/// The scene and battle selections `lbBackupCorrectErrors` checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selections {
    pub fkind: Option<FighterKind>,
    pub training_man_fkind: Option<FighterKind>,
    pub training_com_fkind: Option<FighterKind>,
    pub players: [SelectedPlayer; 4],
    pub maps_vsmode_gkind: u8,
    pub maps_training_gkind: u8,
    /// Set when the US build restores the default item toggles and rate.
    pub items_reset: bool,
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
