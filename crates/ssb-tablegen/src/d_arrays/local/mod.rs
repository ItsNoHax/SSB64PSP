//! Function-local arrays ([`DArray::local`]), one list file per decomp
//! file, named after it (`mn/mnvsmode/mnvsresults.c` is
//! [`mnvsresults`]). Each file has a `GAME` list (into `ssb-game`'s
//! `d_arrays.rs`) and a `ROM` list (into `ssb-rom`'s), in source order,
//! holding only the arrays the Rust code reads.
//!
//! The entries are printed by `tools/local_arrays.py FILE.c --rust`
//! (`FILE.c` relative to `refs/ssb-decomp-re/src`, `--fn`/`--var` to
//! narrow it): it lays the file's local and file-scope arrays out in
//! source order and fits them on the ROM, so the addresses are never
//! transcribed by hand. Copy the entries the Rust code needs; an array
//! the script does not report as `ok` needs a look before it is listed.

use super::DArray;

mod mncharacters;
mod mnplayers1pbonus;
mod mnplayers1pgame;
mod mnplayers1ptraining;
mod mnplayersvs;
mod mnvsrecord;
pub(crate) mod mnvsresults;
mod mvopeningroom;
// agent C
mod lbcommon;
mod mn1pmode;
mod mnbackupclear;
mod mndata;
mod mnmaps;
mod mnmessage;
mod mnoption;
mod mnvsitemswitch;
mod mnvsmode;
mod mnvsoptions;
mod mvopeningclash;
mod mvopeningdonkey;
mod mvopeningfox;
mod mvopeningkirby;
mod mvopeninglink;
mod mvopeningmario;
mod mvopeningpikachu;
mod mvopeningportraits;
mod mvopeningrun;
mod mvopeningsamus;
mod mvopeningsector;
mod mvopeningyoshi;
mod mvopeningyoster;
mod sc1pintro;
mod sc1pstageclear;
mod scstaffroll;

/// Into `ssb-game`'s `d_arrays.rs`.
pub const GAME: &[&[DArray]] = &[
    mncharacters::GAME,
    mnplayers1pbonus::GAME,
    mnplayers1pgame::GAME,
    mnplayers1ptraining::GAME,
    mnplayersvs::GAME,
    mnvsrecord::GAME,
    mnvsresults::GAME,
    mvopeningroom::GAME,
    // agent C
    mnmaps::GAME,
    mnoption::GAME,
    mndata::GAME,
    mn1pmode::GAME,
    mnvsmode::GAME,
    mnvsoptions::GAME,
    mnvsitemswitch::GAME,
    mnbackupclear::GAME,
    mnmessage::GAME,
    sc1pintro::GAME,
    sc1pstageclear::GAME,
    scstaffroll::GAME,
    mvopeningmario::GAME,
    mvopeningfox::GAME,
    mvopeningdonkey::GAME,
    mvopeningsamus::GAME,
    mvopeninglink::GAME,
    mvopeningyoshi::GAME,
    mvopeningkirby::GAME,
    mvopeningpikachu::GAME,
    mvopeningclash::GAME,
    mvopeningrun::GAME,
    mvopeningyoster::GAME,
    mvopeningportraits::GAME,
    mvopeningsector::GAME,
    lbcommon::GAME,
];

/// Into `ssb-rom`'s `d_arrays.rs`.
pub const ROM: &[&[DArray]] = &[
    mncharacters::ROM,
    mnplayers1pbonus::ROM,
    mnplayers1pgame::ROM,
    mnplayers1ptraining::ROM,
    mnplayersvs::ROM,
    mnvsrecord::ROM,
    mnvsresults::ROM,
    mvopeningroom::ROM,
    // agent C
    mnmaps::ROM,
    mnoption::ROM,
    mndata::ROM,
    mn1pmode::ROM,
    mnvsmode::ROM,
    mnvsoptions::ROM,
    mnvsitemswitch::ROM,
    mnbackupclear::ROM,
    mnmessage::ROM,
    sc1pintro::ROM,
    sc1pstageclear::ROM,
    scstaffroll::ROM,
    mvopeningmario::ROM,
    mvopeningfox::ROM,
    mvopeningdonkey::ROM,
    mvopeningsamus::ROM,
    mvopeninglink::ROM,
    mvopeningyoshi::ROM,
    mvopeningkirby::ROM,
    mvopeningpikachu::ROM,
    mvopeningclash::ROM,
    mvopeningrun::ROM,
    mvopeningyoster::ROM,
    mvopeningportraits::ROM,
    mvopeningsector::ROM,
    lbcommon::ROM,
];
