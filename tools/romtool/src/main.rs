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
//! romtool scene    <rom>          recover DObjDesc scene graphs
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
        ["scene", rom_path, rest @ ..] => scene(rom_path.as_ref(), rest),
        ["pack", rom_path, rest @ ..] => pack(rom_path.as_ref(), rest),
        ["texdump", rom_path, rest @ ..] => texdump(rom_path.as_ref(), rest),
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
    romtool scene    <rom.z64> [--file <id>] [--list] [--nodes] [--why]
                               [--expect <ground-truth.tsv>]
    romtool pack     <rom.z64> [--out <file>] [--file <id>] [--no-swizzle]
    romtool extract  <rom.z64> [--out <dir>] [--limit <n>]
    romtool dump     <rom.z64> <file-id>
    romtool textures <rom.z64>
    romtool texdump  <rom.z64> [--file <id>] [--count <n>]

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
/// Recovers `DObjDesc` scene graphs, optionally checking them against the
/// arrays the decomp has typed by hand.
///
/// The `--expect` file is TSV of `file<TAB>offset<TAB>entries<TAB>name`,
/// generated from `refs/ssb-decomp-re/src/relocData/*.c`. It is ground truth in
/// the strongest available sense: those declarations are byte-compared against
/// the original ROM on every decomp build, so an offset in that list is a place
/// a `DObjDesc` array provably starts.
fn scene(path: &Path, args: &[&str]) -> Res {
    use ssb_rom::scene;

    let mut only_file: Option<u32> = None;
    let mut expect: Option<PathBuf> = None;
    let mut list = false;
    let mut nodes = false;
    let mut why = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "--file" => only_file = it.next().map(|v| v.parse()).transpose()?,
            "--expect" => expect = it.next().map(PathBuf::from),
            "--list" => list = true,
            "--nodes" => nodes = true,
            "--why" => why = true,
            other => return Err(format!("unknown flag {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;

    let ids: Vec<u32> = match only_file {
        Some(id) => vec![id],
        None => (0..archive.len() as u32).collect(),
    };

    // file -> offset -> node count (terminator excluded)
    let mut found: BTreeMap<u32, BTreeMap<u32, usize>> = BTreeMap::new();
    let mut total_nodes = 0usize;
    let mut with_dl = 0usize;
    let mut depth_hist: BTreeMap<u32, usize> = BTreeMap::new();

    for id in &ids {
        let Ok(file) = archive.load(*id) else {
            continue;
        };
        let graphs = scene::find_scene_graphs(&file);
        if graphs.is_empty() {
            continue;
        }
        let per_file = found.entry(*id).or_default();
        for g in &graphs {
            per_file.insert(g.offset, g.nodes.len());
            total_nodes += g.nodes.len();
            with_dl += g.nodes.iter().filter(|n| n.desc.dl.is_some()).count();
            for n in &g.nodes {
                *depth_hist.entry(n.desc.depth()).or_default() += 1;
            }
        }
    }

    let graph_count: usize = found.values().map(BTreeMap::len).sum();
    println!("Scene graphs: {graph_count} across {} files", found.len());
    println!("Nodes:        {total_nodes} ({with_dl} carrying a display list)");
    print!("Depths:      ");
    for (d, n) in &depth_hist {
        print!(" {d}:{n}");
    }
    println!();

    if only_file.is_some() || list {
        for (id, graphs) in &found {
            for (off, n) in graphs {
                println!("  file {id} @ 0x{off:X}: {n} nodes");
            }
        }
    }

    // Per-node world positions, for checking composed transforms against the
    // translate values in the decomp's DObjDesc arrays by hand.
    if nodes {
        for id in &ids {
            let Ok(file) = archive.load(*id) else {
                continue;
            };
            for g in scene::find_scene_graphs(&file) {
                println!("\nfile {id} @ 0x{:X} ({} nodes)", g.offset, g.nodes.len());
                for (i, (node, w)) in g.nodes.iter().zip(g.world_transforms()).enumerate() {
                    let t = w.translation();
                    println!(
                        "  {i:3}  depth {}  parent {:>4}  world ({:>10.1} {:>9.1} {:>9.1})  dl {}",
                        node.desc.depth(),
                        node.parent.map_or("-".into(), |p| p.to_string()),
                        t[0],
                        t[1],
                        t[2],
                        node.desc.dl.map_or("-".into(), |d| format!("0x{d:X}")),
                    );
                }
            }
        }
    }

    // Report what each node's `dl` actually resolves to. A DObj's display-list
    // field is a union -- Gfx*, Gfx**, DObjDLLink*, an animation joint -- and
    // nothing in the data discriminates it, so "does it convert" is what
    // decides how much geometry a graph can actually place.
    let mut outcome: BTreeMap<String, usize> = BTreeMap::new();
    let mut members: BTreeMap<&'static str, usize> = BTreeMap::new();
    for id in &ids {
        let Ok(file) = archive.load(*id) else {
            continue;
        };
        let resolver = scene::DlResolver::new(&file);
        for g in scene::find_scene_graphs(&file) {
            for node_dl in g.display_lists() {
                let member = resolver.resolve(node_dl);
                *members
                    .entry(match member {
                        scene::NodeDl::Links(_) => "DObjDLLink[]",
                        scene::NodeDl::Pair { .. } => "Gfx *dls[2] pre/post pair",
                        scene::NodeDl::Direct(_) => "Gfx * (direct)",
                    })
                    .or_default() += 1;
            }

            // Convert the graph the way the packer does -- in draw order,
            // sharing one vertex cache -- so this reports what actually
            // happens rather than what a standalone conversion would.
            let plan = plan_draw_order(&g, &resolver);
            let decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
                .iter()
                .map(|p| {
                    file.data
                        .get(p.dl as usize..)
                        .and_then(|d| ssb_rom::dl::decode_list(d).ok())
                        .unwrap_or_default()
                })
                .collect();
            let items: Vec<ssb_rom::mesh::SequenceItem> = plan
                .iter()
                .zip(&decoded)
                .map(|(p, cmds)| ssb_rom::mesh::SequenceItem {
                    cmds,
                    world: p.world,
                })
                .collect();

            for (p, converted) in plan
                .iter()
                .zip(ssb_rom::mesh::convert_sequence(&items, &file.data))
            {
                let key: String = match converted {
                    _ if p.dl == NO_LIST => "no list on this side of the matrix".into(),
                    Err(e) => format!("convert failed: {e:?}"),
                    Ok(m) if m.triangle_count() == 0 => "converted, no triangles".into(),
                    Ok(_) => "converted with triangles".into(),
                };
                if why && !key.starts_with("converted with") {
                    println!("  WHY file {id} node {} -> 0x{:X}: {key}", p.node, p.dl);
                }
                *outcome.entry(key).or_default() += 1;
            }
        }
    }
    println!("\nUnion member each node's `dl` turned out to be:");
    for (k, n) in &members {
        println!("  {n:5}  {k}");
    }

    let resolved: usize = outcome.values().sum();
    println!("\nNode display lists ({resolved} after union resolution):");
    for (k, n) in &outcome {
        println!("  {n:5}  {k}");
    }

    let Some(expect_path) = expect else {
        return Ok(());
    };

    let text = fs::read_to_string(&expect_path)?;
    let (mut matched, mut wrong_len, mut missing) = (0usize, 0usize, 0usize);
    let mut expected_total = 0usize;

    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let cols: Vec<&str> = line.split('\t').collect();
        let (file, offset, entries) = (
            cols[0].parse::<u32>()?,
            cols[1].parse::<u32>()?,
            cols[2].parse::<usize>()?,
        );
        let name = cols.get(3).copied().unwrap_or("");
        expected_total += 1;

        // The decomp counts the terminator as an entry; we do not.
        let want_nodes = entries - 1;
        match found.get(&file).and_then(|g| g.get(&offset)) {
            Some(&got) if got == want_nodes => matched += 1,
            Some(&got) => {
                wrong_len += 1;
                println!("  LEN  file {file} @ 0x{offset:X} {name}: got {got}, want {want_nodes}");
            }
            None => {
                missing += 1;
                println!("  MISS file {file} @ 0x{offset:X} {name}: {want_nodes} nodes");
            }
        }
    }

    // Everything we found that the decomp has not typed. These are not errors:
    // only 96 of the 2132 files have had their DObjDesc arrays annotated, so
    // most extras are real arrays nobody has labelled yet. The number is worth
    // watching for sudden growth, which would mean the filters loosened.
    let extra = graph_count - matched - wrong_len;

    println!();
    println!(
        "Against {expected_total} annotated arrays in {}:",
        expect_path.display()
    );
    println!("  exact match:   {matched}");
    println!("  wrong length:  {wrong_len}");
    println!("  not found:     {missing}");
    println!("  unannotated:   {extra}");

    if missing > 0 || wrong_len > 0 {
        return Err(format!(
            "{} annotated arrays did not round-trip",
            missing + wrong_len
        )
        .into());
    }
    Ok(())
}

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
    // Does a vertex's colour field actually hold a unit normal? N64 normals are
    // i8 components of a unit vector, so x^2+y^2+z^2 lands near 127^2 = 16129.
    // Colours have no reason to. This distinguishes "the geometry mode said
    // lit" from "the data is normals", which is the question that matters.
    let mut lit_verts = 0usize;
    let mut unlit_verts = 0usize;
    let mut lit_normal_like = 0usize;
    let mut unlit_normal_like = 0usize;
    let normal_like = |c: [u8; 4]| {
        let x = c[0] as i8 as i32;
        let y = c[1] as i8 as i32;
        let z = c[2] as i8 as i32;
        let m = x * x + y * y + z * z;
        (11000..=21000).contains(&m)
    };

    for file in &files {
        let all = ssb_rom::scan::find_root_display_lists(file);

        // convert() inlines G_DL callees, so a discovered list that another
        // discovered list calls would be counted twice. Keep only true roots.
        let called: std::collections::BTreeSet<u32> =
            all.iter().flat_map(|d| d.referenced_lists()).collect();

        for dl in all.iter().filter(|d| !called.contains(&d.offset)) {
            match mesh::convert(&dl.commands, &file.data) {
                Ok(m) => {
                    for prim in &m.primitives {
                        let mut idx: Vec<u16> = prim.indices.clone();
                        idx.sort_unstable();
                        idx.dedup();
                        for i in idx {
                            let Some(v) = m.vertices.get(i as usize) else {
                                continue;
                            };
                            let nl = normal_like(v.rgba);
                            if prim.material.lit {
                                lit_verts += 1;
                                lit_normal_like += nl as usize;
                            } else {
                                unlit_verts += 1;
                                unlit_normal_like += nl as usize;
                            }
                        }
                    }
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

    println!("normal-vs-colour analysis");
    println!("  vertices in lit prims    {lit_verts}");
    println!("  vertices in unlit prims  {unlit_verts}");
    println!(
        "  unlit that look like unit normals: {unlit_normal_like} ({:.1}%)",
        unlit_normal_like as f64 / unlit_verts.max(1) as f64 * 100.0
    );
    println!(
        "  lit   that look like unit normals: {lit_normal_like} ({:.1}%)",
        lit_normal_like as f64 / lit_verts.max(1) as f64 * 100.0
    );

    println!("\nmesh conversion");
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

/// One display list a scene graph draws, and the space it draws it in.
struct PlannedList {
    /// Index of the owning node within its graph.
    node: usize,
    /// File-relative offset of the display list.
    dl: u32,
    /// Modelview in effect while the list runs: the index of the node whose
    /// matrix is current, or `None` for the object root.
    ///
    /// Usually the node's own. A `Gfx *dls[2]` pair's first list draws *before*
    /// the node's matrix is pushed, so it runs in the parent's space instead.
    space: Option<usize>,
    world: ssb_rom::scene::Mat4,
}

impl PlannedList {
    /// Whether this list can be placed on the node itself. A node holds one
    /// mesh, so anything else has to become an extra leaf; see
    /// `PackWriter::add_object`.
    fn own_space(&self) -> bool {
        self.space == Some(self.node)
    }
}

/// A node with no display list still occupies a slot in the draw sequence, so
/// that vertex-cache state lines up with the game's own walk.
const NO_LIST: u32 = u32::MAX;

/// Flattens a graph into the order `gcDrawDObjTree*` would draw it.
///
/// That order is simply the `DObjDesc` array order: `gcAddChildForDObj` appends
/// each new node to the tail of its parent's sibling list, and the draw walk is
/// node-then-child-then-siblings, so the pre-order flattening the array already
/// is round-trips exactly. Nothing needs sorting.
fn plan_draw_order(
    graph: &ssb_rom::scene::SceneGraph,
    resolver: &ssb_rom::scene::DlResolver,
) -> Vec<PlannedList> {
    use ssb_rom::scene::{Mat4, NodeDl};

    let worlds = graph.world_transforms();
    let mut out = Vec::with_capacity(graph.nodes.len());

    for (i, node) in graph.nodes.iter().enumerate() {
        let Some(node_dl) = node.desc.dl else {
            continue;
        };
        let own = |dl| PlannedList {
            node: i,
            dl,
            space: Some(i),
            world: worlds[i],
        };
        match resolver.resolve(node_dl) {
            NodeDl::Direct(dl) => out.push(own(dl)),
            NodeDl::Links(links) => out.extend(links.iter().filter_map(|l| l.dl).map(own)),
            NodeDl::Pair { pre, post } => {
                if let Some(dl) = pre {
                    out.push(PlannedList {
                        node: i,
                        dl,
                        space: node.parent,
                        world: node.parent.map_or(Mat4::IDENTITY, |p| worlds[p]),
                    });
                }
                // The node's matrix is pushed between the two, so even when
                // `post` is NULL the node still occupies a step in the walk.
                out.push(own(post.unwrap_or(NO_LIST)));
            }
        }
    }
    out
}

/// Adds a converted mesh to the pack, uploading any textures it samples.
fn pack_mesh(
    writer: &mut ssb_rom::pack::PackWriter,
    tex_index: &mut BTreeMap<(u32, u32), u32>,
    file: &ssb_rom::archive::File,
    id: u32,
    offset: u32,
    m: &ssb_rom::mesh::Mesh,
    swizzle: bool,
) -> u32 {
    let mut per_prim: Vec<Option<u32>> = Vec::with_capacity(m.primitives.len());
    for prim in &m.primitives {
        per_prim.push(match prim.material.texture {
            None => None,
            Some(t) => {
                let key = (id, t.data_offset);
                if let Some(&i) = tex_index.get(&key) {
                    Some(i)
                } else {
                    convert_texture(&file.data, &t, swizzle).map(|tex| {
                        let i = writer.add_texture(&tex);
                        tex_index.insert(key, i);
                        i
                    })
                }
            }
        });
    }
    writer.add_mesh(m, id, offset, |i| per_prim[i])
}

/// Builds the runtime asset pack: converted geometry and textures in the
/// layout the PSP consumes directly.
fn pack(path: &Path, opts: &[&str]) -> Res {
    use ssb_rom::{mesh, pack as fmt};

    let mut out_path = PathBuf::from("assets/generated/ssb64.pak");
    let mut only_file: Option<u32> = None;
    let mut swizzle = true;

    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--out" => out_path = it.next().ok_or("--out needs a path")?.into(),
            "--file" => only_file = Some(parse_id(it.next().ok_or("--file needs an id")?)?),
            // For bisecting on-device texture bugs: swizzling is a pure
            // reordering, so if output changes with it off, the swizzler or the
            // GE's swizzle flag is at fault rather than the decode.
            "--no-swizzle" => swizzle = false,
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
    let mut objects = 0usize;
    let mut placed_meshes = 0usize;
    let mut node_dls = 0usize;
    let mut extra_leaves = 0usize;

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

        // A scene-graph node's `dl` is an *authoritative* list start: the game
        // itself passes that pointer to `gcAddDObjForGObj`. Blind discovery is
        // a heuristic by comparison, and its outermost-list reduction actively
        // discards these -- a hierarchy's per-joint lists sit inside the span
        // of a larger list the scan preferred. So convert the authoritative
        // offsets first and let discovery fill in only what they miss.
        let graphs = ssb_rom::scene::find_scene_graphs(&file);
        // A node's `dl` may be a `Gfx*`, a `DObjDLLink[]` or a pre/post pair;
        // the resolver sorts that out.
        let resolver = ssb_rom::scene::DlResolver::new(&file);

        // Every list a graph draws, in the order the game draws it, so the
        // vertex cache can be threaded through them; see convert_sequence.
        let plans: Vec<Vec<PlannedList>> = graphs
            .iter()
            .map(|g| plan_draw_order(g, &resolver))
            .collect();
        let authoritative: std::collections::BTreeSet<u32> = plans
            .iter()
            .flatten()
            .map(|p| p.dl)
            .filter(|&d| d != NO_LIST)
            .collect();

        let mut node_mesh: Vec<Vec<Option<u32>>> =
            graphs.iter().map(|g| vec![None; g.nodes.len()]).collect();
        // Geometry a node cannot hold itself: extra link entries, and the
        // pre-matrix half of a pair, which draws in the parent's space.
        let mut node_extra: Vec<Vec<(Option<usize>, u32)>> =
            graphs.iter().map(|_| Vec::new()).collect();

        for (gi, plan) in plans.iter().enumerate() {
            // Decode authoritative offsets *directly*, not through
            // `find_display_lists_at`: that re-applies the discovery
            // heuristics, and a heuristic can only lose information once the
            // game itself has told us where the list starts. Routing them
            // through it placed 742 of 1661 node lists; decoding them straight
            // places 1417.
            let decoded: Vec<Vec<ssb_rom::dl::Cmd>> = plan
                .iter()
                .map(|p| {
                    file.data
                        .get(p.dl as usize..)
                        .and_then(|d| ssb_rom::dl::decode_list(d).ok())
                        .unwrap_or_default()
                })
                .collect();
            let items: Vec<mesh::SequenceItem> = plan
                .iter()
                .zip(&decoded)
                .map(|(p, cmds)| mesh::SequenceItem {
                    cmds,
                    world: p.world,
                })
                .collect();

            for (p, converted) in plan.iter().zip(mesh::convert_sequence(&items, &file.data)) {
                let Ok(m) = converted else { continue };
                if m.triangle_count() == 0 {
                    continue;
                }
                let index = pack_mesh(&mut writer, &mut tex_index, &file, id, p.dl, &m, swizzle);
                meshes += 1;
                triangles += m.triangle_count();

                // A node holds one mesh index, but a link array or a pre/post
                // pair can name several lists. The one drawn under the node's
                // own matrix goes on the node; the rest become extra leaves in
                // whichever space they actually run in.
                let slot = &mut node_mesh[gi][p.node];
                if p.own_space() && slot.is_none() {
                    *slot = Some(index);
                } else {
                    node_extra[gi].push((p.space, index));
                    extra_leaves += 1;
                }
            }
        }

        // Discovery fills in only what the graphs never named.
        for dl in all
            .iter()
            .filter(|d| !called.contains(&d.offset) && !authoritative.contains(&d.offset))
        {
            let Ok(m) = mesh::convert(&dl.commands, &file.data) else {
                continue;
            };
            if m.triangle_count() == 0 {
                continue;
            }
            pack_mesh(
                &mut writer,
                &mut tex_index,
                &file,
                id,
                dl.offset,
                &m,
                swizzle,
            );
            meshes += 1;
            triangles += m.triangle_count();
        }

        for (gi, graph) in graphs.iter().enumerate() {
            node_dls += graph.display_lists().count();
            placed_meshes += node_mesh[gi].iter().filter(|m| m.is_some()).count();
            writer.add_object(graph, id, |n| node_mesh[gi][n], &node_extra[gi]);
            objects += 1;
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
    // One GE draw call per primitive, so this is the number the state-sorting
    // in `merge_by_material` exists to hold down.
    println!(
        "  draws       {} ({:.1} triangles each)",
        pack.prim_count(),
        triangles as f64 / pack.prim_count().max(1) as f64
    );
    println!("  textures    {}", pack.texture_count());
    println!(
        "  objects     {objects} ({} nodes, {placed_meshes}/{node_dls} node lists placed)",
        pack.node_count()
    );
    if extra_leaves > 0 {
        println!(
            "  extra       {extra_leaves} leaf nodes for lists a node could not hold \
             (extra link entries, and pre-matrix pair halves drawn in the parent's space)"
        );
    }
    println!("  size        {:.1} KiB", bytes.len() as f64 / 1024.0);
    println!("  verified    loads back cleanly");

    // Rank objects by the geometry they actually place. This is the number the
    // on-device viewer opens on, and it is the honest measure of how much of
    // the scene graph is doing useful work.
    let mut ranked: Vec<(u32, u32, u32)> = Vec::new();
    for i in 0..pack.object_count() {
        let Some(o) = pack.object(i) else { continue };
        let mut tris = 0u32;
        for k in 0..o.node_count {
            let Some(n) = pack.node(o.first_node + k) else {
                continue;
            };
            let Some(m) = (n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
                .then(|| pack.mesh(n.mesh))
                .flatten()
            else {
                continue;
            };
            for j in 0..m.prim_count {
                if let Some(pr) = pack.prim(m.first_prim + j) {
                    tris += pr.index_count / 3;
                }
            }
        }
        ranked.push((i, tris, o.source_file));
    }
    ranked.sort_by_key(|&(_, t, _)| core::cmp::Reverse(t));
    let placed_tris: u32 = ranked.iter().map(|&(_, t, _)| t).sum();
    println!(
        "  in objects  {placed_tris} triangles ({:.0}% of the pack)",
        placed_tris as f64 / triangles.max(1) as f64 * 100.0
    );
    println!("  top objects:");
    for (i, t, f) in ranked.iter().take(5) {
        println!("    object {i:<4} file {f:<5} {t} triangles");
    }
    Ok(())
}

/// Decodes and packs one texture referenced by a primitive.
fn convert_texture(
    file: &[u8],
    t: &ssb_rom::mesh::TextureRef,
    swizzle: bool,
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
        psp::pack_paletted(src, t.width as u32, t.height as u32, t.size, &tlut, swizzle).ok()
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
        .map(|img| psp::pack_rgba(&img, psp::Psm::Psm8888, swizzle))
    }
}

/// Writes decoded textures out as PPM so they can be eyeballed.
///
/// The point is to separate two very different failures: a texture that
/// converts correctly on the host but renders as noise on device means the
/// bug is in swizzling, format or upload; one that is already noise here means
/// the bug is upstream, in the offsets or palettes the display list gave us.
fn texdump(path: &Path, opts: &[&str]) -> Res {
    use ssb_rom::{mesh, texture};

    let mut only_file: Option<u32> = None;
    let mut count = 12usize;
    let mut out_dir = PathBuf::from("assets/generated/texdump");

    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--file" => only_file = Some(parse_id(it.next().ok_or("--file needs an id")?)?),
            "--count" => count = it.next().ok_or("--count needs a number")?.parse()?,
            "--out" => out_dir = it.next().ok_or("--out needs a dir")?.into(),
            other => return Err(format!("unknown option {other}").into()),
        }
    }

    let (data, info) = load_rom(path)?;
    let archive = Archive::open(&data, info.region)?;
    fs::create_dir_all(&out_dir)?;

    let mut written = 0usize;
    let mut seen: std::collections::BTreeSet<(u32, u32)> = std::collections::BTreeSet::new();

    for id in 0..archive.len() as u32 {
        if written >= count {
            break;
        }
        if only_file.is_some_and(|f| f != id) {
            continue;
        }
        let Ok(file) = archive.load(id) else { continue };

        for dl in ssb_rom::scan::find_root_display_lists(&file) {
            if written >= count {
                break;
            }
            let Ok(m) = mesh::convert(&dl.commands, &file.data) else {
                continue;
            };
            for prim in &m.primitives {
                if written >= count {
                    break;
                }
                let Some(t) = prim.material.texture else {
                    continue;
                };
                if !seen.insert((id, t.data_offset)) {
                    continue;
                }
                if (t.data_offset >> 24) != 0 || t.data_offset == 0 {
                    continue;
                }

                let need = texture::data_len(t.width as u32, t.height as u32, t.size);
                let Some(src) = file
                    .data
                    .get(t.data_offset as usize..t.data_offset as usize + need)
                else {
                    continue;
                };
                let tlut: Vec<u16> = match t.palette_offset {
                    Some(off) => {
                        let n = t.palette_entries.max(1) as usize;
                        file.data
                            .get(off as usize..off as usize + n * 2)
                            .map(texture::parse_tlut)
                            .unwrap_or_default()
                    }
                    None => Vec::new(),
                };

                let Ok(img) = texture::decode(
                    src,
                    t.width as u32,
                    t.height as u32,
                    t.format,
                    t.size,
                    (!tlut.is_empty()).then_some(tlut.as_slice()),
                ) else {
                    continue;
                };

                // PPM: trivially writable without a PNG dependency, and
                // ImageMagick can convert it for viewing.
                let mut ppm = format!("P6\n{} {}\n255\n", img.width, img.height).into_bytes();
                for px in img.pixels.as_chunks::<4>().0 {
                    ppm.extend_from_slice(&px[..3]);
                }
                let name = format!(
                    "f{id}_o{:X}_{:?}{}_{}x{}.ppm",
                    t.data_offset,
                    t.format,
                    t.size.bits(),
                    t.width,
                    t.height
                );
                fs::write(out_dir.join(&name), &ppm)?;
                println!("  {name}  palette {} entries", tlut.len());
                written += 1;
            }
        }
    }

    println!("wrote {written} textures to {}", out_dir.display());
    Ok(())
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
