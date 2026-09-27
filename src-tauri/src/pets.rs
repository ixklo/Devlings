//! Codex-compatible pet packages: a folder with `pet.json` and a PNG or WebP spritesheet.

use std::{
    collections::HashSet,
    ffi::OsString,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::store::DEFAULT_PET_ID;

pub const MAX_SPRITE_BYTES: u64 = 20 * 1024 * 1024;
/// The Codex atlas: 8 columns x 9 rows of 192x208 cells.
pub const SPRITE_SIZE: (u32, u32) = (1536, 1872);
/// Enough of a PNG or WebP file to read its dimensions.
const HEADER_BYTES: usize = 30;
const MANIFEST: &str = "pet.json";
const DEFAULT_SPRITES: [&str; 2] = ["spritesheet.webp", "spritesheet.png"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PetSource {
    Bundled,
    Perch,
    Codex,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub source: PetSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pet {
    pub info: PetInfo,
    /// The pet's folder; the sprite must stay inside it.
    pub dir: PathBuf,
    pub sprite: PathBuf,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Manifest {
    id: Option<String>,
    display_name: Option<String>,
    description: Option<String>,
    spritesheet_path: Option<String>,
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// A spritesheet path from a manifest: relative, and never leaving the pet's folder.
fn safe_relative(rel: &str) -> Option<PathBuf> {
    let p = Path::new(rel);
    let ok = !rel.trim().is_empty() && p.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    ok.then(|| p.to_path_buf())
}

fn is_sprite_ext(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("webp"))
}

/// Width and height from a PNG IHDR chunk or a WebP VP8, VP8L or VP8X header.
pub fn image_size(bytes: &[u8]) -> Result<(u32, u32), String> {
    let be32 = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let le16 = |at: usize| bytes.get(at..at + 2).map(|b| u32::from(u16::from_le_bytes([b[0], b[1]])));
    let le24 = |at: usize| bytes.get(at..at + 3).map(|b| u32::from_le_bytes([b[0], b[1], b[2], 0]));
    let truncated = || "the image header is incomplete".to_string();
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        if bytes.get(12..16) != Some(b"IHDR") {
            return Err("the PNG has no IHDR chunk first".into());
        }
        return Ok((be32(16).ok_or_else(truncated)?, be32(20).ok_or_else(truncated)?));
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return match bytes.get(12..16) {
            // Lossy: frame tag, start code 9d 01 2a, then 14-bit sizes (the top two bits are a scale).
            Some(b"VP8 ") => {
                if bytes.get(23..26) != Some(&[0x9d, 0x01, 0x2a][..]) {
                    return Err("the WebP VP8 frame has no start code".into());
                }
                Ok((le16(26).ok_or_else(truncated)? & 0x3fff, le16(28).ok_or_else(truncated)? & 0x3fff))
            }
            // Lossless: signature 0x2f, then 14-bit width-1 and height-1.
            Some(b"VP8L") => {
                if bytes.get(20) != Some(&0x2f) {
                    return Err("the WebP VP8L stream has no signature".into());
                }
                let bits = bytes.get(21..25).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(truncated)?;
                Ok(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
            }
            // Extended: 24-bit canvas width-1 and height-1.
            Some(b"VP8X") => Ok((le24(24).ok_or_else(truncated)? + 1, le24(27).ok_or_else(truncated)? + 1)),
            Some(_) => Err("the WebP has an unknown first chunk".into()),
            None => Err(truncated()),
        };
    }
    Err("not a PNG or WebP image".into())
}

fn check_sprite(path: &Path) -> Result<(), String> {
    if !is_sprite_ext(path) {
        return Err(format!("{} is not a .png or .webp file", path.display()));
    }
    let meta = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }
    if meta.len() > MAX_SPRITE_BYTES {
        return Err(format!("{} is larger than 20 MiB", path.display()));
    }
    let mut header = Vec::with_capacity(HEADER_BYTES);
    fs::File::open(path)
        .and_then(|f| f.take(HEADER_BYTES as u64).read_to_end(&mut header))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let (w, h) = image_size(&header).map_err(|e| format!("{}: {e}", path.display()))?;
    if (w, h) != SPRITE_SIZE {
        return Err(format!(
            "{} is {w}x{h}; pet spritesheets must be {}x{}",
            path.display(),
            SPRITE_SIZE.0,
            SPRITE_SIZE.1
        ));
    }
    Ok(())
}

/// A sprite path must be a regular file (not a symlink) that resolves inside the pet's folder.
fn check_inside(dir: &Path, sprite: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(sprite).map_err(|e| format!("{}: {e}", sprite.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!("{}: the spritesheet can't be a symlink", sprite.display()));
    }
    let real = |p: &Path| fs::canonicalize(p).map_err(|e| format!("{}: {e}", p.display()));
    if !real(sprite)?.starts_with(real(dir)?) {
        return Err(format!("{}: the spritesheet must be inside the pet folder", sprite.display()));
    }
    Ok(())
}

/// Reads and validates one pet folder.
pub fn load_pet(dir: &Path, source: PetSource) -> Result<Pet, String> {
    let text = fs::read_to_string(dir.join(MANIFEST)).map_err(|e| format!("{}: {e}", dir.display()))?;
    let m: Manifest = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("{}: bad {MANIFEST}: {e}", dir.display()))?;
    let folder = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let id = non_empty(m.id).unwrap_or(folder);
    if id.is_empty() || id.chars().count() > 64 || id.chars().any(char::is_control) {
        return Err(format!("{}: invalid pet id", dir.display()));
    }
    let sprite = match non_empty(m.spritesheet_path) {
        Some(rel) => {
            let rel = safe_relative(&rel).ok_or_else(|| format!("{}: spritesheetPath must stay inside the pet folder", dir.display()))?;
            dir.join(rel)
        }
        None => DEFAULT_SPRITES
            .iter()
            .map(|n| dir.join(n))
            .find(|p| p.is_file())
            .ok_or_else(|| format!("{}: no spritesheetPath and no spritesheet file", dir.display()))?,
    };
    check_inside(dir, &sprite)?;
    check_sprite(&sprite)?;
    Ok(Pet {
        dir: dir.to_path_buf(),
        info: PetInfo {
            display_name: non_empty(m.display_name).unwrap_or_else(|| id.clone()),
            description: non_empty(m.description).unwrap_or_default(),
            id,
            source,
        },
        sprite,
    })
}

fn rank(s: PetSource) -> u8 {
    match s {
        PetSource::Bundled => 0,
        PetSource::Perch => 1,
        PetSource::Codex => 2,
    }
}

/// Every valid pet in the given roots. Earlier roots win when ids repeat. Missing roots and broken pets are skipped.
pub fn discover(roots: &[(PetSource, PathBuf)]) -> Vec<Pet> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (source, root) in roots {
        let Ok(entries) = fs::read_dir(root) else { continue };
        let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        dirs.sort();
        for dir in dirs {
            match load_pet(&dir, *source) {
                Ok(pet) if seen.insert(pet.info.id.clone()) => out.push(pet),
                Ok(_) => {}
                Err(e) => log::warn!("Skipping pet: {e}"),
            }
        }
    }
    // Grouped by source; the default pet leads its group so it shows first in pickers, and the
    // bundled pets keep their collection order (other pets stay in folder order).
    out.sort_by_key(|p| (rank(p.info.source), p.info.id != DEFAULT_PET_ID, bundled_position(p)));
    out
}

/// The pets that ship with Perch, in the order pickers and the menu list them: the three birds,
/// then the other species. `scripts/pets` builds exactly these (its `PET_IDS`).
pub const BUNDLED_ORDER: [&str; 9] = ["perch", "ember", "plum", "fox", "cat", "axolotl", "capybara", "robot", "ghost"];

fn bundled_position(p: &Pet) -> usize {
    if p.info.source != PetSource::Bundled {
        return usize::MAX;
    }
    BUNDLED_ORDER.iter().position(|id| *id == p.info.id).unwrap_or(usize::MAX)
}

/// The wanted pet, else the default pet, else the first one available.
pub fn resolve<'a>(pets: &'a [Pet], wanted: &str) -> Option<&'a Pet> {
    let by_id = |id: &str| pets.iter().find(|p| p.info.id == id);
    by_id(wanted).or_else(|| by_id(DEFAULT_PET_ID)).or_else(|| pets.first())
}

pub fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// The pet's sprite as a data URL. The folder is checked again here: it may have changed since discovery.
pub fn sprite_data_url(pet: &Pet) -> Result<String, String> {
    check_inside(&pet.dir, &pet.sprite)?;
    check_sprite(&pet.sprite)?;
    read_data_url(&pet.sprite)
}

fn read_data_url(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    let mime = sniff_mime(&bytes).ok_or_else(|| format!("{} is not a PNG or WebP image", path.display()))?;
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// `${CODEX_HOME:-~/.codex}`.
pub fn codex_home(env: Option<OsString>, home: &Path) -> PathBuf {
    env.filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".codex"))
}

/// Pet roots in merge order: bundled resources (plus the source tree in debug builds), Perch's own folder, then Codex's.
pub fn roots(resource_dir: Option<PathBuf>, data_dir: &Path, codex_home: &Path) -> Vec<(PetSource, PathBuf)> {
    let mut out = Vec::new();
    if let Some(r) = resource_dir {
        out.push((PetSource::Bundled, r.join("pets")));
    }
    if cfg!(debug_assertions) {
        out.push((PetSource::Bundled, PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/pets"))));
    }
    out.push((PetSource::Perch, user_pets_dir(data_dir)));
    out.push((PetSource::Codex, codex_home.join("pets")));
    out
}

pub fn user_pets_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("pets")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_of(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        b.extend(w.to_be_bytes());
        b.extend(h.to_be_bytes());
        b.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
        b
    }

    fn riff(chunk: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut b = b"RIFF".to_vec();
        b.extend((4 + 8 + data.len() as u32).to_le_bytes());
        b.extend(b"WEBP");
        b.extend(chunk);
        b.extend((data.len() as u32).to_le_bytes());
        b.extend(data);
        b
    }

    fn webp_lossless(w: u32, h: u32) -> Vec<u8> {
        let bits = (w - 1) | ((h - 1) << 14);
        let mut data = vec![0x2f];
        data.extend(bits.to_le_bytes());
        riff(b"VP8L", &data)
    }

    fn webp_lossy(w: u16, h: u16) -> Vec<u8> {
        let mut data = vec![0x10, 0x02, 0x00, 0x9d, 0x01, 0x2a];
        data.extend(w.to_le_bytes());
        data.extend(h.to_le_bytes());
        riff(b"VP8 ", &data)
    }

    fn webp_extended(w: u32, h: u32) -> Vec<u8> {
        let mut data = vec![0x10, 0, 0, 0];
        data.extend(&(w - 1).to_le_bytes()[..3]);
        data.extend(&(h - 1).to_le_bytes()[..3]);
        riff(b"VP8X", &data)
    }

    fn png() -> Vec<u8> {
        png_of(SPRITE_SIZE.0, SPRITE_SIZE.1)
    }

    fn webp() -> Vec<u8> {
        webp_lossless(SPRITE_SIZE.0, SPRITE_SIZE.1)
    }

    fn make_pet(root: &Path, folder: &str, manifest: &str, sprite: Option<(&str, &[u8])>) -> PathBuf {
        let dir = root.join(folder);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(MANIFEST), manifest).unwrap();
        if let Some((name, bytes)) = sprite {
            fs::write(dir.join(name), bytes).unwrap();
        }
        dir
    }

    fn manifest(id: &str, sprite: &str) -> String {
        format!(r#"{{"id":"{id}","displayName":"{id} name","description":"d","spritesheetPath":"{sprite}"}}"#)
    }

    fn ids(pets: &[Pet]) -> Vec<(&str, PetSource)> {
        pets.iter().map(|p| (p.info.id.as_str(), p.info.source)).collect()
    }

    #[test]
    fn loads_a_codex_manifest() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_pet(t.path(), "ember", &manifest("ember", "spritesheet.png"), Some(("spritesheet.png", &png())));
        let pet = load_pet(&dir, PetSource::Bundled).unwrap();
        assert_eq!(pet.info.display_name, "ember name");
        assert_eq!(pet.sprite, dir.join("spritesheet.png"));
        let v = serde_json::to_value(&pet.info).unwrap();
        assert_eq!(v, serde_json::json!({"id":"ember","displayName":"ember name","description":"d","source":"bundled"}));
    }

    #[test]
    fn fills_in_missing_fields() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_pet(t.path(), "blob", "\u{feff}{}", Some(("spritesheet.webp", &webp())));
        let pet = load_pet(&dir, PetSource::Codex).unwrap();
        assert_eq!((pet.info.id.as_str(), pet.info.display_name.as_str(), pet.info.description.as_str()), ("blob", "blob", ""));
        assert_eq!(pet.sprite, dir.join("spritesheet.webp"));
    }

    #[test]
    fn rejects_broken_pets() {
        let t = tempfile::tempdir().unwrap();
        let bad_json = make_pet(t.path(), "a", "{nope", Some(("s.png", &png())));
        assert!(load_pet(&bad_json, PetSource::Perch).is_err());
        let no_sprite = make_pet(t.path(), "b", &manifest("b", "missing.png"), None);
        assert!(load_pet(&no_sprite, PetSource::Perch).is_err());
        let wrong_ext = make_pet(t.path(), "c", &manifest("c", "s.gif"), Some(("s.gif", &png())));
        assert!(load_pet(&wrong_ext, PetSource::Perch).is_err());
        let escapes = make_pet(t.path(), "d", &manifest("d", "../a/s.png"), None);
        assert!(load_pet(&escapes, PetSource::Perch).is_err());
        assert!(load_pet(&t.path().join("nothing"), PetSource::Perch).is_err());
    }

    #[test]
    fn rejects_sprites_over_20_mib() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_pet(t.path(), "big", &manifest("big", "s.png"), Some(("s.png", &png())));
        let f = fs::OpenOptions::new().write(true).open(dir.join("s.png")).unwrap();
        f.set_len(MAX_SPRITE_BYTES + 1).unwrap();
        assert!(load_pet(&dir, PetSource::Perch).unwrap_err().contains("20 MiB"));
        f.set_len(MAX_SPRITE_BYTES).unwrap();
        assert!(load_pet(&dir, PetSource::Perch).is_ok());
    }

    #[test]
    fn relative_paths() {
        assert!(safe_relative("sheet.png").is_some());
        assert!(safe_relative("./art/sheet.png").is_some());
        assert!(safe_relative("../sheet.png").is_none());
        assert!(safe_relative("a/../../sheet.png").is_none());
        assert!(safe_relative("/etc/sheet.png").is_none());
        assert!(safe_relative("").is_none());
        #[cfg(windows)]
        assert!(safe_relative("C:\\sheet.png").is_none());
    }

    #[test]
    fn discovers_in_order_and_skips_duplicates() {
        let t = tempfile::tempdir().unwrap();
        let (bundled, user, codex) = (t.path().join("b"), t.path().join("u"), t.path().join("c"));
        make_pet(&bundled, "plum", &manifest("plum", "s.png"), Some(("s.png", &png())));
        make_pet(&bundled, "perch", &manifest("perch", "s.png"), Some(("s.png", &png())));
        make_pet(&bundled, "ember", &manifest("ember", "s.png"), Some(("s.png", &png())));
        make_pet(&user, "mine", &manifest("mine", "s.webp"), Some(("s.webp", &webp())));
        make_pet(&user, "perch-copy", &manifest("perch", "s.png"), Some(("s.png", &png())));
        make_pet(&codex, "broken", "{", None);
        make_pet(&codex, "dewey", &manifest("dewey", "s.webp"), Some(("s.webp", &webp())));
        let roots = vec![
            (PetSource::Bundled, bundled.clone()),
            (PetSource::Bundled, t.path().join("missing")),
            (PetSource::Perch, user),
            (PetSource::Codex, codex),
        ];
        let pets = discover(&roots);
        assert_eq!(
            ids(&pets),
            vec![
                ("perch", PetSource::Bundled),
                ("ember", PetSource::Bundled),
                ("plum", PetSource::Bundled),
                ("mine", PetSource::Perch),
                ("dewey", PetSource::Codex),
            ]
        );
        assert_eq!(pets[0].sprite, bundled.join("perch").join("s.png"));
        assert!(discover(&[(PetSource::Bundled, t.path().join("missing"))]).is_empty());
    }

    fn shipped_pets() -> Vec<Pet> {
        discover(&[(PetSource::Bundled, PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/pets")))])
    }

    #[test]
    fn the_bundled_pets_come_in_collection_order() {
        // What the pickers and the right-click menu list: the three birds, then the six other species.
        let pets = shipped_pets();
        let ids: Vec<&str> = pets.iter().map(|p| p.info.id.as_str()).collect();
        assert_eq!(ids, BUNDLED_ORDER);
        assert_eq!(ids.len(), 9);
        assert_eq!(ids[0], DEFAULT_PET_ID);
        let names: Vec<&str> = pets.iter().map(|p| p.info.display_name.as_str()).collect();
        assert_eq!(names, ["Perch", "Ember", "Plum", "Pip", "Miso", "Nori", "Bean", "Bolt", "Wisp"]);
    }

    #[test]
    fn unknown_bundled_ids_follow_the_known_ones() {
        let t = tempfile::tempdir().unwrap();
        for id in ["zeta", "fox", "alpha", "perch", "cat"] {
            make_pet(t.path(), id, &manifest(id, "s.png"), Some(("s.png", &png())));
        }
        let pets = discover(&[(PetSource::Bundled, t.path().to_path_buf())]);
        let order: Vec<&str> = ids(&pets).into_iter().map(|(id, _)| id).collect();
        assert_eq!(order, ["perch", "fox", "cat", "alpha", "zeta"]);
    }

    #[test]
    fn resolves_with_fallbacks() {
        let pet = |id: &str| Pet {
            dir: PathBuf::new(),
            info: PetInfo { id: id.into(), display_name: id.into(), description: String::new(), source: PetSource::Bundled },
            sprite: PathBuf::new(),
        };
        let all = vec![pet("ember"), pet("perch")];
        assert_eq!(resolve(&all, "ember").unwrap().info.id, "ember");
        assert_eq!(resolve(&all, "gone").unwrap().info.id, "perch");
        let no_default = vec![pet("plum"), pet("ember")];
        assert_eq!(resolve(&no_default, "gone").unwrap().info.id, "plum");
        assert!(resolve(&[], "perch").is_none());
    }

    #[test]
    fn data_urls_use_the_real_image_type() {
        let t = tempfile::tempdir().unwrap();
        let a = make_pet(t.path(), "a", &manifest("a", "s.png"), Some(("s.png", &png())));
        let pet = load_pet(&a, PetSource::Perch).unwrap();
        assert!(sprite_data_url(&pet).unwrap().starts_with("data:image/png;base64,iVBORw0KGgo"));
        // A WebP saved with a .png extension still gets the WebP type.
        let b = make_pet(t.path(), "b", &manifest("b", "s.png"), Some(("s.png", &webp())));
        assert!(sprite_data_url(&load_pet(&b, PetSource::Perch).unwrap()).unwrap().starts_with("data:image/webp;base64,UklGR"));
        // Changed after discovery: checked again when read.
        fs::write(a.join("s.png"), b"hello").unwrap();
        assert!(sprite_data_url(&pet).is_err());
        fs::write(a.join("s.png"), png_of(10, 10)).unwrap();
        assert!(sprite_data_url(&pet).is_err());
    }

    #[test]
    fn a_sprite_swapped_for_a_symlink_after_discovery_is_refused() {
        let t = tempfile::tempdir().unwrap();
        let outside = t.path().join("outside.png");
        fs::write(&outside, png()).unwrap();
        let dir = make_pet(t.path(), "p", &manifest("p", "s.png"), Some(("s.png", &png())));
        let pet = load_pet(&dir, PetSource::Perch).unwrap();
        fs::remove_file(dir.join("s.png")).unwrap();
        if symlink(&outside, &dir.join("s.png")).is_err() {
            eprintln!("skipping: can't create symlinks here");
            return;
        }
        assert!(sprite_data_url(&pet).unwrap_err().contains("symlink"));
    }

    #[test]
    fn codex_home_honors_the_environment() {
        let home = Path::new("/home/u");
        assert_eq!(codex_home(None, home), home.join(".codex"));
        assert_eq!(codex_home(Some(OsString::new()), home), home.join(".codex"));
        assert_eq!(codex_home(Some("/opt/codex".into()), home), PathBuf::from("/opt/codex"));
    }

    #[test]
    fn roots_are_in_merge_order() {
        let r = roots(Some("/res".into()), Path::new("/data"), Path::new("/cx"));
        assert_eq!(r.first(), Some(&(PetSource::Bundled, PathBuf::from("/res").join("pets"))));
        let tail: Vec<_> = r.iter().rev().take(2).cloned().collect();
        assert_eq!(tail, vec![(PetSource::Codex, Path::new("/cx").join("pets")), (PetSource::Perch, Path::new("/data").join("pets"))]);
        assert_eq!(roots(None, Path::new("/data"), Path::new("/cx")).len(), if cfg!(debug_assertions) { 3 } else { 2 });
    }

    #[test]
    fn reads_png_and_webp_dimensions() {
        assert_eq!(image_size(&png_of(1536, 1872)), Ok((1536, 1872)));
        assert_eq!(image_size(&png_of(7, 9)), Ok((7, 9)));
        assert_eq!(image_size(&webp_lossless(1536, 1872)), Ok((1536, 1872)));
        assert_eq!(image_size(&webp_lossless(1, 16384)), Ok((1, 16384)));
        assert_eq!(image_size(&webp_lossy(1536, 1872)), Ok((1536, 1872)));
        // The top two bits of each VP8 dimension are a scale, not part of the size.
        assert_eq!(image_size(&webp_lossy(1536 | 0x4000, 1872 | 0xc000)), Ok((1536, 1872)));
        assert_eq!(image_size(&webp_extended(1536, 1872)), Ok((1536, 1872)));
        assert_eq!(image_size(&webp_extended(16_777_216, 1)), Ok((16_777_216, 1)));
        assert!(image_size(b"").is_err());
        assert!(image_size(b"hello world, this is not an image").is_err());
        assert!(image_size(&png()[..20]).is_err());
        assert!(image_size(&webp()[..22]).is_err());
        let mut bad_png = png();
        bad_png[12..16].copy_from_slice(b"IDAT");
        assert!(image_size(&bad_png).is_err());
        let mut bad_lossless = webp();
        bad_lossless[20] = 0;
        assert!(image_size(&bad_lossless).is_err());
        let mut bad_lossy = webp_lossy(1536, 1872);
        bad_lossy[23] = 0;
        assert!(image_size(&bad_lossy).is_err());
        let mut odd_chunk = webp();
        odd_chunk[12..16].copy_from_slice(b"ALPH");
        assert!(image_size(&odd_chunk).is_err());
    }

    #[test]
    fn rejects_sprites_with_the_wrong_size() {
        let t = tempfile::tempdir().unwrap();
        let small = make_pet(t.path(), "small", &manifest("small", "s.png"), Some(("s.png", &png_of(192, 208))));
        let err = load_pet(&small, PetSource::Perch).unwrap_err();
        assert!(err.contains("1536x1872") && err.contains("192x208"), "{err}");
        let lossy = make_pet(t.path(), "lossy", &manifest("lossy", "s.webp"), Some(("s.webp", &webp_lossy(1536, 1872))));
        assert!(load_pet(&lossy, PetSource::Perch).is_ok());
        let ext = make_pet(t.path(), "ext", &manifest("ext", "s.webp"), Some(("s.webp", &webp_extended(1536, 1871))));
        assert!(load_pet(&ext, PetSource::Perch).is_err());
        let junk = make_pet(t.path(), "junk", &manifest("junk", "s.png"), Some(("s.png", b"not a png")));
        assert!(load_pet(&junk, PetSource::Perch).is_err());
        let small_pet = Pet { dir: small.clone(), sprite: small.join("s.png"), ..load_pet(&lossy, PetSource::Perch).unwrap() };
        assert!(sprite_data_url(&small_pet).is_err());
    }

    #[test]
    fn rejects_paths_that_leave_the_folder() {
        let t = tempfile::tempdir().unwrap();
        make_pet(t.path(), "other", &manifest("other", "s.png"), Some(("s.png", &png())));
        let abs = serde_json::to_string(&t.path().join("other").join("s.png").display().to_string()).unwrap();
        let absolute = make_pet(t.path(), "abs", &format!(r#"{{"spritesheetPath":{abs}}}"#), None);
        assert!(load_pet(&absolute, PetSource::Perch).unwrap_err().contains("inside the pet folder"));
        let dotdot = make_pet(t.path(), "dots", &manifest("dots", "art/../../other/s.png"), None);
        fs::create_dir_all(dotdot.join("art")).unwrap();
        assert!(load_pet(&dotdot, PetSource::Perch).is_err());
        let nested = make_pet(t.path(), "nested", &manifest("nested", "art/s.png"), None);
        fs::create_dir_all(nested.join("art")).unwrap();
        fs::write(nested.join("art").join("s.png"), png()).unwrap();
        assert_eq!(load_pet(&nested, PetSource::Perch).unwrap().sprite, nested.join("art").join("s.png"));
    }

    #[cfg(unix)]
    fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }

    #[test]
    fn rejects_symlinked_sprites() {
        let t = tempfile::tempdir().unwrap();
        let outside = t.path().join("outside.png");
        fs::write(&outside, png()).unwrap();
        let linked = make_pet(t.path(), "linked", &manifest("linked", "s.png"), None);
        if symlink(&outside, &linked.join("s.png")).is_err() {
            // Creating symlinks needs Developer Mode or admin rights on Windows.
            eprintln!("skipping: can't create symlinks here");
            return;
        }
        assert!(load_pet(&linked, PetSource::Perch).unwrap_err().contains("symlink"));
        // A default spritesheet name that is a symlink is refused too.
        let default_name = make_pet(t.path(), "dflt", "{}", None);
        symlink(&outside, &default_name.join("spritesheet.png")).unwrap();
        assert!(load_pet(&default_name, PetSource::Perch).is_err());
        // A symlinked folder inside the pet that points elsewhere resolves outside.
        let art = t.path().join("elsewhere");
        fs::create_dir_all(&art).unwrap();
        fs::write(art.join("s.png"), png()).unwrap();
        let via_dir = make_pet(t.path(), "viadir", &manifest("viadir", "art/s.png"), None);
        symlink(&art, &via_dir.join("art")).unwrap();
        assert!(load_pet(&via_dir, PetSource::Perch).unwrap_err().contains("inside the pet folder"));
    }
}
