//! The ROM's audio files (`ssb_rom::audio`) against R1 §5.1's census of the
//! US ROM: the section builds, parses, and holds the expected counts.

use ssb_engine::audio::data::{self, Bank, Part, Section};
use ssb_rom::audio;

fn rom() -> Option<Vec<u8>> {
    Some(std::fs::read(std::env::var_os("SSB64_ROM")?).unwrap())
}

#[test]
fn audio_section_counts() {
    let Some(rom) = rom() else {
        return;
    };
    let bytes = audio::build_section(&rom).unwrap();
    let s = Section::open(&bytes).unwrap();

    let seqs = data::parse_sbk(s.part(Part::Sbk)).unwrap();
    assert_eq!(seqs.len(), 47);
    assert_eq!(seqs.iter().map(|q| q.len).max(), Some(13_366));

    // B1_sounds1: 43 instArray entries (entry 0 is offset 0, the file
    // header), percussion with 25 sounds, 119 sounds, 117 waves and books,
    // 72 loops, all ADPCM.
    let music = Bank::parse(s.part(Part::MusicCtl), s.part(Part::MusicTbl).len()).unwrap();
    assert_eq!(music.sample_rate, 32000);
    assert_eq!(music.inst_array.len(), 43);
    let perc = music.percussion.unwrap() as usize;
    assert_eq!(music.instruments[perc].sounds.len(), 25);
    assert_eq!(music.sounds.len(), 119);
    assert_eq!(music.wavetables.len(), 117);
    assert_eq!(music.books.len(), 117);
    assert_eq!(music.loops.len(), 72);
    assert!(music
        .wavetables
        .iter()
        .all(|w| w.kind == data::AL_ADPCM_WAVE));
    let vibrato = music.instruments.iter().filter(|i| i.vib_type != 0).count();
    assert_eq!(vibrato, 33);

    // B1_sounds2: one instrument of 322 sounds, 26 loops.
    let sfx = Bank::parse(s.part(Part::SfxCtl), s.part(Part::SfxTbl).len()).unwrap();
    assert_eq!(sfx.sample_rate, 44100);
    assert_eq!(sfx.inst_array.len(), 1);
    assert_eq!(sfx.sounds.len(), 322);
    assert_eq!(sfx.wavetables.len(), 322);
    assert_eq!(sfx.loops.len(), 26);
    assert!(sfx.books.iter().all(|b| b.order == 2 && b.npredictors == 4));

    assert_eq!(data::fgm_unk_count(s.part(Part::FgmUnk)).unwrap(), 100);
    assert_eq!(
        data::parse_package(s.part(Part::FgmTbl)).unwrap().len(),
        464
    );
    assert_eq!(
        data::parse_package(s.part(Part::FgmUcd)).unwrap().len(),
        695
    );

    // The constant tables' known shapes: eqpower is a quarter cosine from
    // 32767 to 0; the sine table starts 0, 50, 100 and peaks at 32768.
    let eq: [i16; 128] = data::be_i16s(s.part(Part::EqPower));
    assert_eq!((eq[0], eq[127]), (32767, 0));
    let sin = s.part(Part::SinTable);
    assert_eq!(&sin[..6], &[0, 0, 0, 50, 0, 100]);
    let fx: [i32; 114] = data::be_i32s(s.part(Part::CustomFx));
    assert_eq!((fx[0], fx[1]), (14, 19200));
    // SMALLROOM_PARAMS_N: 3 sections, 100 ms (x 40).
    let presets: [i32; 100] = data::be_i32s(s.part(Part::Presets));
    assert_eq!((presets[0], presets[1], presets[26]), (3, 4000, 4));
}

#[test]
fn audio_range_finds_the_section() {
    let mut w = ssb_rom::pack::PackWriter::new();
    w.set_audio(vec![7; 100]);
    let bytes = w.finish();
    let (off, len) = ssb_rom::pack::audio_range(&bytes).unwrap();
    assert_eq!(off % ssb_rom::pack::AUDIO_ALIGN, 0);
    assert_eq!(&bytes[off..off + len], &[7; 100][..]);
    assert_eq!(off + len, bytes.len());
    let resident = ssb_rom::pack::resident_len(&bytes).unwrap();
    assert!(resident <= off);
    assert!(ssb_rom::pack::audio_range(&ssb_rom::pack::PackWriter::new().finish()).is_none());
}
