//! Build-time ROM validation and asset extraction.
//!
//! The user supplies their own legally owned `.z64`. This tool reads it and
//! writes derived assets into `assets/generated/`, which is gitignored. The
//! ROM is never copied into the repository, and neither is anything extracted
//! from it.
//!
//! ```text
//! romtool verify   <rom>          identify the ROM and print its header
//! romtool info     <rom>          summarise the relocData archive
//! romtool extract  <rom> [--out]  extract every archive file + a manifest
//! romtool dump     <rom> <id>     dump one archive file
//! romtool textures <rom>          extract + pack every bound texture
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ssb_rom::archive::Archive;
use ssb_rom::rom;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();

    let result = match refs.as_slice() {
        ["verify", rom_path] => verify(rom_path.as_ref()),
        ["info", rom_path] => info(rom_path.as_ref()),
        ["check", rom_path] => check(rom_path.as_ref()),
        ["scan", rom_path, rest @ ..] => scan(rom_path.as_ref(), rest),
        ["mesh", rom_path] => mesh(rom_path.as_ref()),
        ["pack", rom_path, rest @ ..] => pack(rom_path.as_ref(), rest),
        ["extract", rom_path, rest @ ..] => extract(rom_path.as_ref(), rest),
        ["dump", rom_path, id] => dump(rom_path.as_ref(), id),
        ["textures", rom_path] => textures(rom_path.as_ref()),
        _ => {
            usage();
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!(
        "romtool -- SSB64 ROM validation and asset extraction

USAGE:
    romtool verify   <rom.z64>
    romtool info     <rom.z64>
    romtool check    <rom.z64>
    romtool scan     <rom.z64> [--exhaustive]
    romtool mesh     <rom.z64>
    romtool pack     <rom.z64> [--out <file>] [--file <id>]
    romtool extract  <rom.z64> [--out <dir>] [--limit <n>]
    romtool dump     <rom.z64> <file-id>
    romtool textures <rom.z64>

The ROM is read only. Output defaults to assets/generated/, which is
gitignored -- no extracted asset is ever committed."
    );
}

type Res = Result<(), Box<dyn std::error::Error>>;

fn load_rom(path: &Path) -> Result<(Vec<u8>, rom::RomInfo), Box<dyn std::error::Error>> {
    let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let info = rom::identify(&data)?;
    Ok((data, info))
}

fn verify(path: &Path) -> Res {
    let (_, info) = load_rom(path)?;
    println!("ROM:      {}", path.display());
    println!("Region:   {:?}", info.region);
    println!("Name:     {}", info.internal_name);
    println!(
        "Code:     {}",
        String::from_utf8_lossy(info.region.game_code())
    );
    println!("SHA-1:    {}", info.sha1);
    println!("Status:   supported");
    Ok(())
}

fn info(path: &Path) -> Res {
    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let entries = archive.entries();
    let compressed = entries.iter().filter(|e| e.compressed).count();
    let total_rom: usize = entries.iter().map(|e| e.rom_size()).sum();
    let total_ram: usize = entries.iter().map(|e| e.size()).sum();
    let largest = entries
        .iter()
        .enumerate()
        .max_by_key(|(_, e)| e.size())
        .expect("archive is non-empty");

    println!("relocData archive");
    println!("  table offset   0x{:08X}", archive.table_offset());
    println!("  data base      0x{:08X}", archive.data_base());
    println!("  files          {}", archive.len());
    println!(
        "  compressed     {compressed} ({:.1}%)",
        compressed as f64 / archive.len() as f64 * 100.0
    );
    println!(
        "  packed size    {:.2} MiB",
        total_rom as f64 / (1 << 20) as f64
    );
    println!(
        "  unpacked size  {:.2} MiB",
        total_ram as f64 / (1 << 20) as f64
    );
    println!(
        "  ratio          {:.2}x",
        total_ram as f64 / total_rom as f64
    );
    println!(
        "  largest file   #{} at {} KiB",
        largest.0,
        largest.1.size() / 1024
    );
    Ok(())
}

/// Validates every archive file, cross-checking decompression against ROM
/// geometry. See `Archive::verify_extern_chain` for why this is a meaningful
/// test of the VPK0 decoder and not just a smoke test.
fn check(path: &Path) -> Res {
    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let mut load_failures = Vec::new();
    let mut chain_mismatches = Vec::new();
    let mut compressed_verified = 0usize;
    let mut total_extern = 0usize;
    let mut total_intern = 0usize;

    for id in 0..archive.len() as u32 {
        let compressed = archive.entry(id).is_some_and(|e| e.compressed);

        match archive.verify_extern_chain(id) {
            Ok((chain, id_list)) => {
                if chain != id_list {
                    chain_mismatches.push((id, chain, id_list));
                } else if compressed {
                    compressed_verified += 1;
                }
                total_extern += chain;
            }
            Err(e) => load_failures.push((id, e)),
        }

        if let Ok(f) = archive.load(id) {
            total_intern += f.intern_relocs.len();
        }
    }

    println!("archive self-check");
    println!("  files                 {}", archive.len());
    println!("  load failures         {}", load_failures.len());
    println!("  intern reloc slots    {total_intern}");
    println!("  extern reloc slots    {total_extern}");
    println!("  chain/ROM mismatches  {}", chain_mismatches.len());
    println!("  compressed files cross-verified against ROM geometry: {compressed_verified}");

    for (id, e) in load_failures.iter().take(10) {
        eprintln!("  load failure: file {id}: {e}");
    }
    for (id, chain, list) in chain_mismatches.iter().take(10) {
        eprintln!("  mismatch: file {id}: chain has {chain}, ROM implies {list}");
    }

    if load_failures.is_empty() && chain_mismatches.is_empty() {
        println!("\nOK: every file decompressed and relocated consistently.");
        Ok(())
    } else {
        Err("archive self-check failed".into())
    }
}

/// Inventories every display list in the archive.
///
/// This is the measurement plan §8 asks for: rather than speculatively
/// supporting the whole RDP feature set, find out what Smash actually emits and
/// build a converter for that.
fn scan(path: &Path, opts: &[&str]) -> Res {
    let how = if opts.contains(&"--exhaustive") {
        ssb_rom::scan::Candidates::Exhaustive
    } else {
        ssb_rom::scan::Candidates::RelocTargets
    };
    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    // Load everything once, then build the cross-file pointer graph. A display
    // list can live in one file and be referenced only from another, so
    // scanning files in isolation misses them.
    let files: Vec<_> = (0..archive.len() as u32)
        .filter_map(|id| archive.load(id).ok())
        .collect();
    let mut inv = ssb_rom::scan::Inventory::default();
    let mut biggest: Vec<(u32, usize)> = Vec::new();

    for file in &files {
        let dls = if how == ssb_rom::scan::Candidates::Exhaustive {
            ssb_rom::scan::find_display_lists_with(file, how)
        } else {
            ssb_rom::scan::find_root_display_lists(file)
        };

        if !dls.is_empty() {
            let tris: usize = dls.iter().map(|d| d.triangle_count()).sum();
            biggest.push((file.id, tris));
        }
        inv.add_file(&dls);
    }

    println!("display list inventory ({} archive files)", archive.len());
    println!("  files containing DLs   {}", inv.files_with_dls);
    println!("  display lists          {}", inv.display_lists);
    println!("  triangles              {}", inv.triangles);
    println!(
        "  vertex loads           {} ({} vertices)",
        inv.vertex_loads, inv.vertices_loaded
    );
    println!("  max vertices per G_VTX {}", inv.max_vtx_batch);
    println!("  longest DL             {} commands", inv.max_commands);

    println!("\nopcodes actually used:");
    let mut ops: Vec<_> = inv.opcodes.iter().collect();
    ops.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (op, n) in ops {
        println!(
            "  0x{op:02X} {:<18} {n:>8}",
            ssb_rom::scan::opcode_name(*op)
        );
    }

    println!("\ntexture formats (G_SETTILE):");
    if inv.texture_formats.is_empty() {
        println!("  (none)");
    }
    let mut fmts: Vec<_> = inv.texture_formats.iter().collect();
    fmts.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for ((f, s), n) in fmts {
        let name = ssb_rom::texture::Format::from_raw(*f)
            .map(|f| format!("{f:?}"))
            .unwrap_or_else(|| format!("raw{f}"));
        let bits = ssb_rom::texture::BitSize::from_raw(*s)
            .map(|b| b.bits())
            .unwrap_or(0);
        println!("  {name:<5}{bits:>3}bpp  {n:>8}");
    }

    println!("\nTLUT load sizes (palette entries):");
    for (size, n) in &inv.tlut_sizes {
        println!("  {size:>4} entries  {n:>8}");
    }

    println!("\ngeometry mode bits set:");
    for (bit, n) in &inv.geometry_mode_set {
        println!("  0x{bit:08X}  {n:>8}  {}", geometry_mode_name(*bit));
    }

    biggest.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
    println!("\nheaviest files by triangle count:");
    for (id, tris) in biggest.iter().take(12) {
        println!("  file {id:<5} {tris:>7} triangles");
    }

    Ok(())
}

/// F3DEX2 geometry mode bit names (`gbi.h`).
fn geometry_mode_name(bit: u32) -> &'static str {
    match bit {
        0x0000_0001 => "G_ZBUFFER",
        0x0000_0002 => "G_SHADE",
        0x0000_0200 => "G_CULL_FRONT",
        0x0000_0400 => "G_CULL_BACK",
        0x0001_0000 => "G_FOG",
        0x0002_0000 => "G_LIGHTING",
        0x0004_0000 => "G_TEXTURE_GEN",
        0x0008_0000 => "G_TEXTURE_GEN_LINEAR",
        0x0020_0000 => "G_SHADING_SMOOTH",
        0x0100_0000 => "G_CLIPPING",
        _ => "",
    }
}

/// Converts every discovered display list into indexed meshes and reports how
/// well the conversion compresses the geometry.
///
/// The headline number is the vertex dedup ratio: the RSP re-uploads shared
/// vertices constantly because its cache holds only 32, and undoing that is
/// pure win on PSP -- less memory and less GE vertex fetch.
fn mesh(path: &Path) -> Res {
    use ssb_rom::mesh;

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let files: Vec<_> = (0..archive.len() as u32)
        .filter_map(|id| archive.load(id).ok())
        .collect();
    let mut converted = 0usize;
    let mut failed: BTreeMap<String, usize> = BTreeMap::new();
    let mut index_refs = 0usize; // triangle corners
    let mut uniq_vertices = 0usize; // after dedup
    let mut triangles = 0usize;
    let mut draws_after = 0usize; // after merging
    let mut textured = 0usize;

    for file in &files {
        let all = ssb_rom::scan::find_root_display_lists(file);

        // convert() inlines G_DL callees, so a discovered list that another
        // discovered list calls would be counted twice. Keep only true roots.
        let called: std::collections::BTreeSet<u32> =
            all.iter().flat_map(|d| d.referenced_lists()).collect();

        for dl in all.iter().filter(|d| !called.contains(&d.offset)) {
            match mesh::convert(&dl.commands, &file.data) {
                Ok(m) => {
                    converted += 1;
                    uniq_vertices += m.vertex_count();
                    triangles += m.triangle_count();
                    index_refs += m.triangle_count() * 3;
                    draws_after += m.primitives.len();
                    textured += m
                        .primitives
                        .iter()
                        .filter(|p| p.material.texture.is_some())
                        .count();
                }
                Err(e) => *failed.entry(format!("{e:?}")).or_default() += 1,
            }
        }
    }

    println!("mesh conversion");
    println!("  display lists converted  {converted}");
    println!(
        "  failed                   {}",
        failed.values().sum::<usize>()
    );
    for (kind, n) in &failed {
        println!("    {kind:<40} {n}");
    }
    println!("  triangles                {triangles}");
    println!("  triangle corners         {index_refs}");
    println!("  unique vertices          {uniq_vertices}");
    if uniq_vertices > 0 {
        println!(
            "  vertex reuse             {:.2}x",
            index_refs as f64 / uniq_vertices as f64
        );
    }
    println!("  draw calls after merge   {draws_after}");
    println!("  textured draws           {textured}");

    // Geometry memory, comparing the three representations that matter.
    // PSP vertex = 12 bytes at 16-bit components; 24 bytes with float pos/uv.
    let soup_float = index_refs * 24; // expanded triangles, float vertices
    let soup_16 = index_refs * 12; // expanded triangles, 16-bit vertices
    let indexed_16 = uniq_vertices * 12 + index_refs * 2; // + u16 indices
    let kib = |b: usize| b as f64 / 1024.0;
    println!("\ngeometry memory");
    println!("  triangle soup, float     {:>8.1} KiB", kib(soup_float));
    println!("  triangle soup, 16-bit    {:>8.1} KiB", kib(soup_16));
    println!("  indexed, 16-bit          {:>8.1} KiB", kib(indexed_16));
    println!(
        "  saving vs float soup     {:>8.1}%",
        100.0 - (indexed_16 as f64 / soup_float as f64 * 100.0)
    );

    Ok(())
}

/// Builds the runtime asset pack: converted geometry and textures in the
/// layout the PSP consumes directly.
fn pack(path: &Path, opts: &[&str]) -> Res {
    use ssb_rom::{mesh, pack as fmt};

    let mut out_path = PathBuf::from("assets/generated/ssb64.pak");
    let mut only_file: Option<u32> = None;

    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--out" => out_path = it.next().ok_or("--out needs a path")?.into(),
            "--file" => only_file = Some(parse_id(it.next().ok_or("--file needs an id")?)?),
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let mut writer = fmt::PackWriter::new();
    // The same texture is bound by many primitives; upload each once.
    let mut tex_index: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut meshes = 0usize;
    let mut triangles = 0usize;

    for id in 0..archive.len() as u32 {
        if only_file.is_some_and(|f| f != id) {
            continue;
        }
        let Ok(file) = archive.load(id) else { continue };

        let all = ssb_rom::scan::find_root_display_lists(&file);
        // convert() inlines G_DL callees, so packing a list that another list
        // calls would duplicate its geometry.
        let called: std::collections::BTreeSet<u32> =
            all.iter().flat_map(|d| d.referenced_lists()).collect();

        for dl in all.iter().filter(|d| !called.contains(&d.offset)) {
            let Ok(m) = mesh::convert(&dl.commands, &file.data) else {
                continue;
            };
            if m.triangle_count() == 0 {
                continue;
            }

            // Resolve each primitive's texture, adding new ones to the pack.
            let mut per_prim: Vec<Option<u32>> = Vec::with_capacity(m.primitives.len());
            for prim in &m.primitives {
                per_prim.push(match prim.material.texture {
                    None => None,
                    Some(t) => {
                        let key = (id, t.data_offset);
                        if let Some(&i) = tex_index.get(&key) {
                            Some(i)
                        } else {
                            convert_texture(&file.data, &t).map(|tex| {
                                let i = writer.add_texture(&tex);
                                tex_index.insert(key, i);
                                i
                            })
                        }
                    }
                });
            }

            writer.add_mesh(&m, id, dl.offset, |i| per_prim[i]);
            meshes += 1;
            triangles += m.triangle_count();
        }
    }

    let bytes = writer.finish();
    if let Some(dir) = out_path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&out_path, &bytes)?;

    // Verify what we just wrote actually loads, rather than trusting it.
    let pack = ssb_rom::pack::Pack::open(&bytes)
        .map_err(|e| format!("wrote a pack that will not load: {e:?}"))?;

    println!("asset pack -> {}", out_path.display());
    println!("  meshes      {meshes}");
    println!("  triangles   {triangles}");
    println!("  textures    {}", pack.texture_count());
    println!("  size        {:.1} KiB", bytes.len() as f64 / 1024.0);
    println!("  verified    loads back cleanly");
    Ok(())
}

/// Decodes and packs one texture referenced by a primitive.
fn convert_texture(
    file: &[u8],
    t: &ssb_rom::mesh::TextureRef,
) -> Option<ssb_rom::psp_texture::PspTexture> {
    use ssb_rom::psp_texture as psp;
    use ssb_rom::texture;

    if (t.data_offset >> 24) != 0 || t.data_offset == 0 {
        return None; // segmented or extern; not resolvable within this file
    }
    let psm = psp::choose_psm(t.format, t.size);
    let need = texture::data_len(t.width as u32, t.height as u32, t.size);
    let src = file.get(t.data_offset as usize..t.data_offset as usize + need)?;

    let tlut: Vec<u16> = match t.palette_offset {
        Some(off) => {
            let n = t.palette_entries.max(1) as usize;
            file.get(off as usize..off as usize + n * 2)
                .map(texture::parse_tlut)
                .unwrap_or_default()
        }
        None => Vec::new(),
    };

    if psm.is_paletted() && !tlut.is_empty() {
        psp::pack_paletted(src, t.width as u32, t.height as u32, t.size, &tlut, true).ok()
    } else {
        texture::decode(
            src,
            t.width as u32,
            t.height as u32,
            t.format,
            t.size,
            (!tlut.is_empty()).then_some(tlut.as_slice()),
        )
        .ok()
        .map(|img| psp::pack_rgba(&img, psp::Psm::Psm8888, true))
    }
}

fn parse_id(s: &str) -> Result<u32, Box<dyn std::error::Error>> {
    s.strip_prefix("0x")
        .map(|h| u32::from_str_radix(h, 16))
        .unwrap_or_else(|| s.parse())
        .map_err(|e| format!("bad file id {s:?}: {e}").into())
}

fn dump(path: &Path, id: &str) -> Res {
    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;
    let id = parse_id(id)?;

    let entry = archive.entry(id).ok_or("file id out of range")?;
    let file = archive.load(id)?;

    println!("file {id}");
    println!("  compressed     {}", entry.compressed);
    println!(
        "  rom offset     0x{:08X}",
        archive.data_base() + entry.data_offset as usize
    );
    println!("  packed         {} bytes", entry.rom_size());
    println!("  unpacked       {} bytes", file.data.len());
    println!("  intern relocs  {}", file.intern_relocs.len());
    println!("  extern relocs  {}", file.extern_relocs.len());

    let deps: BTreeMap<u16, usize> =
        file.extern_relocs
            .iter()
            .fold(BTreeMap::new(), |mut acc, r| {
                *acc.entry(r.target_file).or_default() += 1;
                acc
            });
    if !deps.is_empty() {
        println!("  depends on:");
        for (target, count) in deps {
            println!("    file {target:<5} ({count} pointer(s))");
        }
    }

    let out = PathBuf::from("assets/generated/dump").join(format!("{id}.bin"));
    fs::create_dir_all(out.parent().unwrap())?;
    fs::write(&out, &file.data)?;
    println!("  written to     {}", out.display());
    Ok(())
}

/// Walks a file's display lists and reports the texture loads it performs.
///
/// This is the reconnaissance step for the rendering pipeline: before writing
/// a converter we need to know which `(format, size)` combinations Smash
/// actually uses, rather than supporting all of them speculatively.
/// Extracts and packs every texture the game's display lists actually bind,
/// then reports the VRAM budget.
///
/// Textures are found via `mesh::convert`, which resolves each primitive's
/// `TextureRef` from the RDP state in force at draw time -- so this covers the
/// textures the game really uses, not every image-shaped blob in the archive.
fn textures(path: &Path) -> Res {
    use ssb_rom::psp_texture as psp;
    use ssb_rom::{mesh, texture};

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let files: Vec<_> = (0..archive.len() as u32)
        .filter_map(|id| archive.load(id).ok())
        .collect();

    // Deduplicate: the same texture is bound by many primitives.
    let mut seen: std::collections::BTreeSet<(u32, u32)> = std::collections::BTreeSet::new();
    let mut by_format: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // count, bytes
    let mut packed_ok = 0usize;
    let mut decode_failed = 0usize;
    let mut total_psp = 0usize;
    let mut total_naive = 0usize;
    let mut swizzled = 0usize;
    let mut largest: Vec<(usize, u32, u32, String)> = Vec::new();
    let mut why: BTreeMap<String, usize> = BTreeMap::new();

    for file in &files {
        for dl in ssb_rom::scan::find_root_display_lists(file) {
            let Ok(m) = mesh::convert(&dl.commands, &file.data) else {
                continue;
            };
            for prim in &m.primitives {
                let Some(t) = prim.material.texture else {
                    continue;
                };
                if !seen.insert((file.id, t.data_offset)) {
                    continue;
                }

                let psm = psp::choose_psm(t.format, t.size);
                let need = texture::data_len(t.width as u32, t.height as u32, t.size);
                let at = t.data_offset as usize;

                // Diagnose *why* a texture cannot be read, rather than lumping
                // every failure together.
                let segment = (t.data_offset >> 24) as u8;
                if segment != 0 {
                    *why.entry(format!("segmented addr (seg 0x{segment:02X})"))
                        .or_default() += 1;
                    decode_failed += 1;
                    continue;
                }
                if t.data_offset == 0 {
                    *why.entry("null (extern reloc, texture in another file)".into())
                        .or_default() += 1;
                    decode_failed += 1;
                    continue;
                }
                let Some(src) = file.data.get(at..at + need) else {
                    *why.entry("offset past end of file".into()).or_default() += 1;
                    decode_failed += 1;
                    continue;
                };
                if psm.is_paletted() && t.palette_offset.is_none() {
                    *why.entry("paletted but no TLUT recorded".into())
                        .or_default() += 1;
                }

                // Palette, if this is a CLUT format.
                let tlut: Vec<u16> = match t.palette_offset {
                    Some(off) => {
                        let entries = t.palette_entries.max(1) as usize;
                        file.data
                            .get(off as usize..off as usize + entries * 2)
                            .map(texture::parse_tlut)
                            .unwrap_or_default()
                    }
                    None => Vec::new(),
                };

                let tex = if psm.is_paletted() && !tlut.is_empty() {
                    match psp::pack_paletted(
                        src,
                        t.width as u32,
                        t.height as u32,
                        t.size,
                        &tlut,
                        true,
                    ) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            *why.entry(format!("pack_paletted: {e:?}")).or_default() += 1;
                            None
                        }
                    }
                } else {
                    match texture::decode(
                        src,
                        t.width as u32,
                        t.height as u32,
                        t.format,
                        t.size,
                        (!tlut.is_empty()).then_some(tlut.as_slice()),
                    ) {
                        Ok(img) => Some(psp::pack_rgba(&img, psp::Psm::Psm8888, true)),
                        Err(e) => {
                            *why.entry(format!("decode: {e:?}")).or_default() += 1;
                            None
                        }
                    }
                };

                match tex {
                    Some(tex) => {
                        packed_ok += 1;
                        if tex.swizzled {
                            swizzled += 1;
                        }
                        let size = tex.vram_size();
                        total_psp += size;
                        // What it would cost expanded to 32-bit RGBA.
                        total_naive += (t.width as usize) * (t.height as usize) * 4;

                        let name = format!("{:?}", tex.format);
                        let e = by_format.entry(name.clone()).or_default();
                        e.0 += 1;
                        e.1 += size;
                        largest.push((size, t.width as u32, t.height as u32, name));
                    }
                    None => decode_failed += 1,
                }
            }
        }
    }

    let kib = |b: usize| b as f64 / 1024.0;

    println!("texture conversion");
    println!("  unique textures bound  {}", seen.len());
    println!("  packed                 {packed_ok}");
    println!("  failed                 {decode_failed}");
    for (reason, n) in &why {
        println!("    {reason:<48} {n:>4}");
    }
    println!(
        "  swizzled               {swizzled} ({:.0}%)",
        swizzled as f64 / packed_ok.max(1) as f64 * 100.0
    );

    println!("\nby PSP format:");
    for (fmt, (n, bytes)) in &by_format {
        println!("  {fmt:<8} {n:>5} textures  {:>9.1} KiB", kib(*bytes));
    }

    println!("\nVRAM budget");
    println!("  packed (chosen formats)  {:>9.1} KiB", kib(total_psp));
    println!("  naive, all RGBA8888      {:>9.1} KiB", kib(total_naive));
    if total_naive > 0 {
        println!(
            "  saving                   {:>9.1}%",
            100.0 - (total_psp as f64 / total_naive as f64 * 100.0)
        );
    }
    // Framebuffers + depth leave roughly this much of the PSP's 2 MiB VRAM.
    const VRAM_FOR_TEXTURES: usize = 700 * 1024;
    println!(
        "  fits in ~700 KiB texture VRAM? {}",
        if total_psp <= VRAM_FOR_TEXTURES {
            "yes, all at once".into()
        } else {
            format!(
                "no - needs streaming ({:.1}x over)",
                total_psp as f64 / VRAM_FOR_TEXTURES as f64
            )
        }
    );

    largest.sort_by_key(|(s, ..)| std::cmp::Reverse(*s));
    println!("\nlargest textures:");
    for (size, w, h, fmt) in largest.iter().take(8) {
        println!("  {w:>3}x{h:<3} {fmt:<8} {:>8.1} KiB", kib(*size));
    }

    Ok(())
}

fn extract(path: &Path, opts: &[&str]) -> Res {
    let mut out_dir = PathBuf::from("assets/generated");
    let mut limit = usize::MAX;

    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--out" => out_dir = it.next().ok_or("--out needs a directory")?.into(),
            "--limit" => {
                limit = it
                    .next()
                    .ok_or("--limit needs a count")?
                    .parse()
                    .map_err(|e| format!("bad --limit: {e}"))?
            }
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let files_dir = out_dir.join("files");
    fs::create_dir_all(&files_dir)?;

    let count = archive.len().min(limit);
    let mut manifest = String::from(
        "# relocData manifest\n\
         # Generated by romtool. Do not commit -- derived from a copyrighted ROM.\n\
         # id\tsize\tcompressed\tintern\textern\tdeps\n",
    );
    let mut failures = Vec::new();
    let mut total = 0usize;

    for id in 0..count as u32 {
        match archive.load(id) {
            Ok(file) => {
                fs::write(files_dir.join(format!("{id:04}.bin")), &file.data)?;
                total += file.data.len();

                let mut deps: Vec<u16> = file.extern_relocs.iter().map(|r| r.target_file).collect();
                deps.sort_unstable();
                deps.dedup();
                let deps = deps
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(",");

                manifest.push_str(&format!(
                    "{id}\t{}\t{}\t{}\t{}\t{}\n",
                    file.data.len(),
                    archive.entry(id).is_some_and(|e| e.compressed),
                    file.intern_relocs.len(),
                    file.extern_relocs.len(),
                    deps
                ));
            }
            Err(e) => failures.push((id, e)),
        }
    }

    fs::write(out_dir.join("manifest.tsv"), manifest)?;

    println!(
        "extracted {}/{count} files to {}",
        count - failures.len(),
        files_dir.display()
    );
    println!("total unpacked: {:.2} MiB", total as f64 / (1 << 20) as f64);

    if !failures.is_empty() {
        eprintln!("\n{} file(s) failed:", failures.len());
        for (id, e) in failures.iter().take(20) {
            eprintln!("  file {id}: {e}");
        }
        if failures.len() > 20 {
            eprintln!("  ... and {} more", failures.len() - 20);
        }
        return Err(format!("{} file(s) failed to extract", failures.len()).into());
    }
    Ok(())
}
