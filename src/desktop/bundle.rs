//! Proof that a package carries Fleet's server exactly as the release built it (`desktop/scripts/check-bundled-server.mjs`,
//! component 25 row 25.2). Every file of the staged server folder is in the package's `fleet/` folder, byte for byte, and
//! nothing else is. On macOS signing rewrites the code signature of each Mach-O file inside; for those every byte before
//! the signature must match, except the four fields signing rewrites: the signature's offset and size
//! (`LC_CODE_SIGNATURE`) and `__LINKEDIT`'s sizes, which grow with the signature. Plain Rust, no Rune; the command line is
//! `examples/check-bundled-server.rs`.
use std::fs;
use std::path::{Path, PathBuf};

const LC_SEGMENT: u32 = 0x1;
const LC_SEGMENT_64: u32 = 0x19;
const LC_CODE_SIGNATURE: u32 = 0x1d;
const MH_MAGIC: u32 = 0xfeed_face;
const MH_MAGIC_64: u32 = 0xfeed_facf;
const FAT_MAGIC: u32 = 0xcafe_babe;

fn le(buf: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(buf.get(at..at + 4)?.try_into().ok()?))
}

fn be(buf: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(buf.get(at..at + 4)?.try_into().ok()?))
}

/// Whether a buffer starts like a (thin little-endian or fat) Mach-O file.
pub fn is_mach_o(buf: &[u8]) -> bool {
    buf.len() >= 8 && (matches!(le(buf, 0), Some(MH_MAGIC | MH_MAGIC_64)) || be(buf, 0) == Some(FAT_MAGIC))
}

/// A thin image cut at its code signature, with the fields signing rewrites zeroed. `Err` says what it couldn't read.
fn mask_thin(image: &[u8]) -> Result<Vec<u8>, String> {
    let magic = le(image, 0).ok_or("too short")?;
    if magic != MH_MAGIC && magic != MH_MAGIC_64 {
        return Err("not a little-endian Mach-O image".into());
    }
    let is64 = magic == MH_MAGIC_64;
    let ncmds = le(image, 16).ok_or("too short")?;
    let mut offset = if is64 { 32 } else { 28 };
    let mut end = image.len();
    let mut masked = image.to_vec();
    let zero = |m: &mut Vec<u8>, from: usize, to: usize| m[from..to].fill(0);
    for _ in 0..ncmds {
        let cmd = le(image, offset).ok_or("load command runs past the file")?;
        let size = le(image, offset + 4).ok_or("load command runs past the file")? as usize;
        if size < 8 || offset + size > image.len() {
            return Err("load command runs past the file".into());
        }
        if cmd == LC_CODE_SIGNATURE {
            end = le(image, offset + 8).ok_or("short signature command")? as usize;
            zero(&mut masked, offset + 8, offset + 16);
        } else if cmd == LC_SEGMENT_64 || cmd == LC_SEGMENT {
            let name = image.get(offset + 8..offset + 24).ok_or("short segment command")?;
            let name = String::from_utf8_lossy(name);
            if name.trim_end_matches('\0') == "__LINKEDIT" {
                let (a, b) = if cmd == LC_SEGMENT_64 { (offset + 32, offset + 48) } else { (offset + 28, offset + 36) };
                let width = if cmd == LC_SEGMENT_64 { 8 } else { 4 };
                if b + width > image.len() {
                    return Err("short segment command".into());
                }
                zero(&mut masked, a, a + width); // vmsize
                zero(&mut masked, b, b + width); // filesize
            }
        }
        offset += size;
    }
    masked.truncate(end.min(image.len()));
    Ok(masked)
}

/// The thin images of a Mach-O file: itself, or each architecture of a fat (universal) file.
fn slices(buf: &[u8]) -> Vec<&[u8]> {
    if be(buf, 0) != Some(FAT_MAGIC) {
        return vec![buf];
    }
    let count = be(buf, 4).unwrap_or(0) as usize;
    (0..count)
        .map(|i| {
            let entry = 8 + i * 20;
            let at = be(buf, entry + 8).unwrap_or(0) as usize;
            let len = be(buf, entry + 12).unwrap_or(0) as usize;
            buf.get(at..at + len).unwrap_or(&[])
        })
        .collect()
}

/// Compares two Mach-O files ignoring only their code signatures (and the size fields that change with them).
/// `None` when they are the same, else what differs.
pub fn compare_mach_o(built: &[u8], shipped: &[u8]) -> Option<String> {
    let (a, b) = (slices(built), slices(shipped));
    if a.len() != b.len() {
        return Some(format!("{} architecture(s) built, {} shipped", a.len(), b.len()));
    }
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        let (x, y) = match (mask_thin(x), mask_thin(y)) {
            (Ok(x), Ok(y)) => (x, y),
            (Err(e), _) | (_, Err(e)) => return Some(format!("slice {i}: {e}")),
        };
        let n = x.len().min(y.len());
        if let Some(first) = (0..n).find(|&k| x[k] != y[k]) {
            return Some(format!("slice {i}: differs at byte {first} (0x{first:x})"));
        }
        // codesign may align the signature differently; anything between must be padding.
        let longer = if x.len() > y.len() { &x } else { &y };
        if longer[n..].iter().any(|&byte| byte != 0) {
            return Some(format!("slice {i}: {} bytes before the signature built, {} shipped, and not just padding", x.len(), y.len()));
        }
    }
    None
}

fn skipped(file: &str) -> bool {
    file.ends_with(".pdb") || file.ends_with(".dbg")
}

/// Every file under `root`, as sorted `/`-separated relative paths.
fn files(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// The package's copies of the server: a `fleet` folder inside a `resources` folder (Fleet's unpacked apps,
/// `Fleet.app/Contents/Resources/fleet`), or any `fleet` folder that holds the server binary (`<app>/fleet` beside the
/// exe, which is where the native app looks first), found within five folders of `root`.
fn bundled_servers(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth > 5 {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let in_resources = dir.file_name().is_some_and(|n| n.eq_ignore_ascii_case("resources"));
            let holds_server = path.join("WeaveFleet.Api").is_file() || path.join("WeaveFleet.Api.exe").is_file();
            if entry.file_name() == "fleet" && (in_resources || holds_server) {
                out.push(path);
            } else {
                walk(&path, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, 0, &mut out);
    out
}

/// The result of a check: whether every copy matched, and the lines to print (errors start with `::error::`).
#[derive(Debug)]
pub struct Check {
    pub ok: bool,
    pub lines: Vec<String>,
}

/// Checks every copy of the server under `release_dir` against `server_dir`.
pub fn check_bundled_server(server_dir: &Path, release_dir: &Path) -> Check {
    let built: Vec<String> = files(server_dir).into_iter().filter(|f| !skipped(f)).collect();
    let copies = bundled_servers(release_dir);
    if copies.is_empty() {
        let line = format!("::error::No resources/fleet folder under {}: was the app packaged?", release_dir.display());
        return Check { ok: false, lines: vec![line] };
    }
    let (mut ok, mut lines) = (true, Vec::new());
    for copy in &copies {
        let shipped = files(copy);
        let missing: Vec<&String> = built.iter().filter(|f| !shipped.contains(f)).collect();
        let extra: Vec<&String> = shipped.iter().filter(|f| !built.contains(f)).collect();
        let mut differ = Vec::new();
        let mut resigned = 0;
        for file in built.iter().filter(|f| shipped.contains(f)) {
            let (Ok(x), Ok(y)) = (fs::read(server_dir.join(file)), fs::read(copy.join(file))) else {
                differ.push(format!("{file} (unreadable)"));
                continue;
            };
            if x == y {
                continue;
            }
            if is_mach_o(&x) && is_mach_o(&y) {
                match compare_mach_o(&x, &y) {
                    None => resigned += 1,
                    Some(why) => differ.push(format!("{file} ({why})")),
                }
            } else {
                differ.push(file.clone());
            }
        }
        let label = copy.strip_prefix(release_dir).unwrap_or(copy).display().to_string();
        let same = built.len() - missing.len() - differ.len();
        let mut line = format!("{label}: {same} of the {} built files shipped as built", built.len());
        if resigned > 0 {
            line += &format!(" ({resigned} re-signed Mach-O files, identical apart from their code signatures)");
        }
        lines.push(line);
        let missing: Vec<String> = missing.into_iter().cloned().collect();
        let extra: Vec<String> = extra.into_iter().cloned().collect();
        for (what, list) in [("missing", missing), ("not from the build", extra), ("different", differ)] {
            if list.is_empty() {
                continue;
            }
            ok = false;
            lines.push(format!(
                "::error::{label}: {} file(s) {what}: {}",
                list.len(),
                list.iter().take(20).cloned().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    Check { ok, lines }
}
