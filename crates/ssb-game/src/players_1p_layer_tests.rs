use super::*;
use crate::players_1p::SceneData;
use crate::spgame::Backup;

fn pieces(s: &Players1P, records: &Records) -> Vec<Draw> {
    let mut out = Vec::new();
    s.visit(records, |d| out.push(d));
    out
}

fn sprite_at(draws: &[Draw], file: u32, offset: u32) -> Vec<(f32, f32)> {
    draws
        .iter()
        .filter_map(|d| match d {
            Draw::Select(vs::Draw::Sprite(p)) if p.file == file && p.offset == offset => {
                Some((p.x, p.y))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn strings_kern_as_the_source() {
    let mut xs = Vec::new();
    string("HIGH SCORE", 142.0, 201.0, [0; 3], &mut |p| xs.push(p.x));
    // H 4+1, I 3+1, G 4+1, H 4+1, space 3, S 4+1, C 4+1, O 4+1, R 4+1.
    assert_eq!(
        xs,
        [142.0, 147.0, 151.0, 156.0, 164.0, 169.0, 174.0, 179.0, 184.0]
    );
    let mut xs = Vec::new();
    // "AT": A before T is tight; T before anything but ' and . is tight.
    string("ATA", 0.0, 0.0, [0; 3], &mut |p| xs.push(p.x));
    assert_eq!(xs, [0.0, 5.0, 10.0]);
}

#[test]
fn fixed_numbers_draw_leading_zeroes_right_to_left() {
    let mut out = Vec::new();
    number(1205, 256.0, 198.0, ([0; 3], [1; 3]), 8, true, &mut |p| {
        out.push((p.offset, p.x))
    });
    assert_eq!(out.len(), 8);
    assert_eq!(out[0], (DIGITS[5], 248.0));
    assert_eq!(out[3], (DIGITS[1], 224.0));
    assert_eq!(out[7], (DIGITS[0], 192.0));
}

#[test]
fn the_level_text_and_stock_icons_follow_the_options() {
    let mut s = Players1P::new(SceneData::default(), &Backup::default());
    s.tick(Default::default(), Default::default());
    let r = Players1P::records(&Backup::default(), None);
    let d = pieces(&s, &r);
    assert_eq!(
        // `dSCManagerDefaultBackupData`'s Easy.
        sprite_at(&d, FILE_DIFFICULTY, LEVEL_TEXT[1]),
        [(219.0, 159.0)]
    );
    // No fighter: three Polygon stocks, left to right at 207, 219, 231.
    let mut zako = sprite_at(&d, FILE_STOCKS_ZAKO, ZAKO_STOCK);
    zako.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(zako, [(207.0, 179.0), (219.0, 179.0), (231.0, 179.0)]);
    // Both arrows show at Easy and 3 stocks.
    assert_eq!(sprite_at(&d, FILE_PLAYERS_COMMON, common::ARROW_L).len(), 2);
    assert_eq!(sprite_at(&d, FILE_PLAYERS_COMMON, common::ARROW_R).len(), 2);
    // A fighter's own icons in its costume.
    s.slot.kind = Some(FighterKind::Fox);
    s.slot.costume = 1;
    let d = pieces(&s, &r);
    let stocks: Vec<_> = d
        .iter()
        .filter(|d| {
            matches!(
                d,
                Draw::Stock {
                    kind: FighterKind::Fox,
                    costume: 1,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(stocks.len(), 3);
}

#[test]
fn the_card_uses_the_ports_lut_and_text() {
    let scene = SceneData {
        player: 2,
        ..SceneData::default()
    };
    let s = Players1P::new(scene, &Backup::default());
    let d = pieces(&s, &Players1P::records(&Backup::default(), None));
    let card = d.iter().find_map(|d| match d {
        Draw::Select(vs::Draw::Sprite(p)) if p.offset == mode::RED_CARD => Some(*p),
        _ => None,
    });
    assert_eq!(
        card.map(|p| (p.lut, p.x, p.y)),
        Some((Some(3), 25.0, 127.0))
    );
    assert_eq!(
        sprite_at(&d, FILE_PLAYERS_COMMON, common::TEXT_1P[2]),
        [(30.0, 132.0)]
    );
}

#[test]
fn records_draw_the_smash_logo_in_the_best_difficultys_colour() {
    let s = Players1P::new(SceneData::default(), &Backup::default());
    let r = Records {
        fighter: Some((0, 0, 5)),
        total_hiscore: 0,
        total_bonuses: 0,
    };
    let d = pieces(&s, &r);
    let logo = d.iter().find_map(|d| match d {
        Draw::Select(vs::Draw::Sprite(p)) if p.offset == mode::SMASH_LOGO => Some(p.prim),
        _ => None,
    });
    assert_eq!(logo, Some(Some(LEVEL_COLORS[4])));
}
