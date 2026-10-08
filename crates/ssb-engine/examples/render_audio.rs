//! Renders the audio system on the host into a WAV under `/tmp/ssb-audio/`
//! and reports the render cost.
//!
//! ```text
//! cargo run --release -p ssb-engine --example render_audio -- \
//!     [--pack assets/generated/ssb64.pak | --rom ROM.z64] \
//!     [--bgm N[ --bgm-at FRAME]] [--bgm-volume V] [--fgm ID[@FRAME[:BALANCE]],...] [--seconds S] [--mono] \
//!     [--bgm-seq ID@FRAME|stop@FRAME,...] [--stop-fgm FRAME,...] [--stop-id ID@FRAME,...] [--frames FILE] [--preroll N] [--out /tmp/ssb-audio/x.wav] [--raw /tmp/ssb-audio/x.raw]
//! ```
//!
//! `--frames FILE` replays a recorded frame-length sequence (the second
//! whitespace-separated column of each line, 552 or 368) instead of the
//! simulated queue, so a render lines up sample for sample with an N64
//! capture; `FRAME` in `--bgm-at` and `--fgm` is then that list's index.
//! `--preroll N` renders and discards N 184-sample sub-frames first: it
//! moves the sequence player's 512-sample tick phase (the N64 creates its
//! player at boot, before the capture starts) without moving the frames.
//!
//! The section comes from the pack's audio section (`audio_range`) or is
//! built from the ROM. Frames follow the N64's 552/368 rhythm, driven by a
//! simulated output queue drained at 32006/60 samples per video frame. The
//! WAV is written at 32006 Hz (`--raw` also writes s16le interleaved PCM).
//! It never writes anywhere but /tmp/ssb-audio.

use std::time::Instant;

use ssb_engine::audio::{AudioSystem, FRAME_SAMPLES_MAX, FRAME_SAMPLES_MIN, N64_OUTPUT_RATE};

struct Args {
    pack: String,
    rom: Option<String>,
    bgm: Option<u32>,
    bgm_volume: Option<u32>,
    bgm_at: u32,
    /// `(id, frame, balance)`: a balance plays it with `func_80026A10`.
    fgms: Vec<(u16, u32, Option<u8>)>,
    seconds: f64,
    mono: bool,
    out: String,
    raw: Option<String>,
    frames: Option<Vec<usize>>,
    preroll: u32,
    /// `(bgm or None = stop, frame)`.
    bgm_seq: Vec<(Option<u32>, u32)>,
    stop_fgm: Vec<u32>,
    /// `(id, frame)`: `func_80026738` on the last handle `--fgm` got for `id`.
    stop_id: Vec<(u16, u32)>,
}

fn parse() -> Result<Args, String> {
    let mut a = Args {
        pack: "assets/generated/ssb64.pak".into(),
        rom: None,
        bgm: None,
        bgm_volume: None,
        bgm_at: 0,
        fgms: Vec::new(),
        seconds: 10.0,
        mono: false,
        out: "/tmp/ssb-audio/render.wav".into(),
        raw: None,
        frames: None,
        preroll: 0,
        bgm_seq: Vec::new(),
        stop_fgm: Vec::new(),
        stop_id: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--pack" => a.pack = val()?,
            "--rom" => a.rom = Some(val()?),
            "--bgm" => a.bgm = Some(val()?.parse().map_err(|e| format!("--bgm: {e}"))?),
            "--bgm-volume" => {
                a.bgm_volume = Some(val()?.parse().map_err(|e| format!("--bgm-volume: {e}"))?)
            }
            "--bgm-at" => a.bgm_at = val()?.parse().map_err(|e| format!("--bgm-at: {e}"))?,
            "--seconds" => a.seconds = val()?.parse().map_err(|e| format!("--seconds: {e}"))?,
            "--mono" => a.mono = true,
            "--bgm-seq" => {
                for item in val()?.split(',') {
                    let (id, at) = item.split_once('@').ok_or(format!("--bgm-seq {item}"))?;
                    let id = match id {
                        "stop" => None,
                        id => Some(id.parse().map_err(|e| format!("--bgm-seq {item}: {e}"))?),
                    };
                    let at = at.parse().map_err(|e| format!("--bgm-seq {item}: {e}"))?;
                    a.bgm_seq.push((id, at));
                }
            }
            "--stop-id" => {
                for item in val()?.split(',') {
                    let (id, at) = item.split_once('@').ok_or(format!("--stop-id {item}"))?;
                    a.stop_id.push((
                        id.parse().map_err(|e| format!("--stop-id {item}: {e}"))?,
                        at.parse().map_err(|e| format!("--stop-id {item}: {e}"))?,
                    ));
                }
            }
            "--stop-fgm" => {
                for item in val()?.split(',') {
                    a.stop_fgm.push(
                        item.parse()
                            .map_err(|e| format!("--stop-fgm {item}: {e}"))?,
                    );
                }
            }
            "--preroll" => a.preroll = val()?.parse().map_err(|e| format!("--preroll: {e}"))?,
            "--out" => a.out = val()?,
            "--raw" => a.raw = Some(val()?),
            "--frames" => {
                let path = val()?;
                let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
                let mut lens = Vec::new();
                for line in text.lines() {
                    let Some(n) = line.split_whitespace().nth(1) else {
                        continue;
                    };
                    let n: usize = n.parse().map_err(|e| format!("{path}: {e}"))?;
                    if n != FRAME_SAMPLES_MAX && n != FRAME_SAMPLES_MIN {
                        return Err(format!("{path}: frame length {n}"));
                    }
                    lens.push(n);
                }
                a.frames = Some(lens);
            }
            "--fgm" => {
                for item in val()?.split(',') {
                    let (id, at) = item.split_once('@').unwrap_or((item, "0"));
                    let (at, bal) = match at.split_once(':') {
                        Some((at, bal)) => (
                            at,
                            Some(bal.parse().map_err(|e| format!("--fgm {item}: {e}"))?),
                        ),
                        None => (at, None),
                    };
                    a.fgms.push((
                        id.parse().map_err(|e| format!("--fgm {item}: {e}"))?,
                        at.parse().map_err(|e| format!("--fgm {item}: {e}"))?,
                        bal,
                    ));
                }
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    for p in std::iter::once(&a.out).chain(a.raw.iter()) {
        if !p.starts_with("/tmp/ssb-audio/") || p.contains("..") {
            return Err("--out and --raw must be under /tmp/ssb-audio/".into());
        }
    }
    Ok(a)
}

fn section(a: &Args) -> Result<&'static [u8], String> {
    let bytes = if let Some(rom) = &a.rom {
        let rom = std::fs::read(rom).map_err(|e| format!("{rom}: {e}"))?;
        ssb_rom::audio::build_section(&rom).map_err(|e| format!("audio section: {e:?}"))?
    } else {
        let pack = std::fs::read(&a.pack).map_err(|e| format!("{}: {e}", a.pack))?;
        let (off, len) = ssb_rom::pack::audio_range(&pack)
            .ok_or("the pack has no audio section (or another VERSION)")?;
        pack[off..off + len].to_vec()
    };
    Ok(Box::leak(bytes.into_boxed_slice()))
}

fn write_wav(path: &str, samples: &[i16]) -> std::io::Result<()> {
    let rate = N64_OUTPUT_RATE as u32;
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::create_dir_all("/tmp/ssb-audio")?;
    std::fs::write(path, out)
}

fn main() {
    let a = match parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("render_audio: {e}");
            std::process::exit(2);
        }
    };
    let section = match section(&a) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("render_audio: {e}");
            std::process::exit(1);
        }
    };
    let mut sys = AudioSystem::new(section).expect("audio section");
    if a.mono {
        sys.set_quality(0);
    }
    if let Some(vol) = a.bgm_volume {
        sys.set_bgm_volume(0, vol);
    }

    let mut frame = [0i16; FRAME_SAMPLES_MAX * 2];
    for _ in 0..a.preroll {
        sys.render_frame(&mut frame, 184);
    }
    let total = (a.seconds * N64_OUTPUT_RATE as f64) as usize;
    let per_vblank = N64_OUTPUT_RATE as f64 / 60.0;
    let mut pcm: Vec<i16> = Vec::with_capacity(total * 2 + FRAME_SAMPLES_MAX * 2);
    let mut handles = Vec::new();
    let mut queued = 0.0f64;
    let mut vblank = 0u32;
    let mut busy = std::time::Duration::ZERO;
    let mut max_voices = 0;
    let mut max_frame = std::time::Duration::ZERO;
    let mut voice_samples = 0u64;
    while pcm.len() / 2 < total {
        if let Some(bgm) = a.bgm {
            if vblank == a.bgm_at {
                sys.play_bgm(0, bgm);
            }
        }
        for &(bgm, at) in &a.bgm_seq {
            if at == vblank {
                match bgm {
                    Some(bgm) => {
                        sys.play_bgm(0, bgm);
                    }
                    None => sys.stop_bgm_all(),
                }
            }
        }
        if a.stop_fgm.contains(&vblank) {
            sys.stop_all_fgm();
        }
        for &(id, at) in &a.stop_id {
            if at == vblank {
                if let Some(h) = handles.iter().rev().find(|(i, _)| *i == id) {
                    sys.stop_fgm(h.1);
                }
            }
        }
        for &(id, at, bal) in &a.fgms {
            if at == vblank {
                let h = match bal {
                    Some(bal) => sys.play_fgm_balance(id, bal),
                    None => sys.play_fgm(id),
                };
                if let Some(h) = h {
                    handles.push((id, h));
                }
            }
        }
        let n = match &a.frames {
            Some(lens) => match lens.get(vblank as usize) {
                Some(&n) => n,
                None => break,
            },
            None => sys.frame_samples(queued as u32),
        };
        let t = Instant::now();
        sys.render_frame(&mut frame, n);
        let dt = t.elapsed();
        busy += dt;
        max_frame = max_frame.max(dt);
        max_voices = max_voices.max(sys.active_voices());
        voice_samples += sys.active_voices() as u64 * n as u64;

        pcm.extend_from_slice(&frame[..n * 2]);
        // The AI drains one video frame's worth before the next retrace.
        queued = (queued + n as f64 - per_vblank).max(0.0);
        vblank += 1;
    }
    let seconds = pcm.len() as f64 / 2.0 / N64_OUTPUT_RATE as f64;
    let peak = pcm.iter().map(|s| (*s as i32).abs()).max().unwrap_or(0);
    let clipped = pcm
        .iter()
        .filter(|s| **s == i16::MAX || **s == i16::MIN)
        .count();
    if let Err(e) = write_wav(&a.out, &pcm) {
        eprintln!("render_audio: {}: {e}", a.out);
        std::process::exit(1);
    }
    if let Some(raw) = &a.raw {
        let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        if let Err(e) = std::fs::write(raw, bytes) {
            eprintln!("render_audio: {raw}: {e}");
            std::process::exit(1);
        }
    }
    println!(
        "{} -> {:.2} s, {} frames, peak {peak}, clipped samples {clipped}, max voices {max_voices}, drops {:?}",
        a.out,
        seconds,
        vblank,
        sys.drops()
    );
    println!(
        "render cost: {:.0} us per second of audio ({:.2} ms per 60 Hz frame, worst frame {} us)",
        busy.as_secs_f64() * 1e6 / seconds,
        busy.as_secs_f64() * 1e3 / vblank as f64,
        max_frame.as_micros()
    );
    if voice_samples > 0 {
        println!(
            "per voice-sample: {:.1} ns ({} voice-samples)",
            busy.as_secs_f64() * 1e9 / voice_samples as f64,
            voice_samples
        );
    }
}
