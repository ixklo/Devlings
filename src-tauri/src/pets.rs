//! Codex-compatible pet packages: a folder with `pet.json` and a PNG or WebP spritesheet.

use std::{
    collections::HashSet,
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
};

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::store::DEFAULT_PET_ID;

pub const MAX_SPRITE_BYTES: u64 = 20 * 1024 * 1024;
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
    check_sprite(&sprite)?;
    Ok(Pet {
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
                Err(e) => eprintln!("Perch: skipping pet: {e}"),
            }
        }
    }
    // Grouped by source; the default pet leads its group so it shows first in pickers.
    out.sort_by_key(|p| (rank(p.info.source), p.info.id != DEFAULT_PET_ID));
    out
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

pub fn sprite_data_url(path: &Path) -> Result<String, String> {
    check_sprite(path)?;
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

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    const WEBP: &[u8] = b"RIFF\x10\0\0\0WEBPVP8L";

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
        let dir = make_pet(t.path(), "ember", &manifest("ember", "spritesheet.png"), Some(("spritesheet.png", PNG)));
        let pet = load_pet(&dir, PetSource::Bundled).unwrap();
        assert_eq!(pet.info.display_name, "ember name");
        assert_eq!(pet.sprite, dir.join("spritesheet.png"));
        let v = serde_json::to_value(&pet.info).unwrap();
        assert_eq!(v, serde_json::json!({"id":"ember","displayName":"ember name","description":"d","source":"bundled"}));
    }

    #[test]
    fn fills_in_missing_fields() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_pet(t.path(), "blob", "\u{feff}{}", Some(("spritesheet.webp", WEBP)));
        let pet = load_pet(&dir, PetSource::Codex).unwrap();
        assert_eq!((pet.info.id.as_str(), pet.info.display_name.as_str(), pet.info.description.as_str()), ("blob", "blob", ""));
        assert_eq!(pet.sprite, dir.join("spritesheet.webp"));
    }

    #[test]
    fn rejects_broken_pets() {
        let t = tempfile::tempdir().unwrap();
        let bad_json = make_pet(t.path(), "a", "{nope", Some(("s.png", PNG)));
        assert!(load_pet(&bad_json, PetSource::Perch).is_err());
        let no_sprite = make_pet(t.path(), "b", &manifest("b", "missing.png"), None);
        assert!(load_pet(&no_sprite, PetSource::Perch).is_err());
        let wrong_ext = make_pet(t.path(), "c", &manifest("c", "s.gif"), Some(("s.gif", PNG)));
        assert!(load_pet(&wrong_ext, PetSource::Perch).is_err());
        let escapes = make_pet(t.path(), "d", &manifest("d", "../a/s.png"), None);
        assert!(load_pet(&escapes, PetSource::Perch).is_err());
        assert!(load_pet(&t.path().join("nothing"), PetSource::Perch).is_err());
    }

    #[test]
    fn rejects_sprites_over_20_mib() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_pet(t.path(), "big", &manifest("big", "s.png"), None);
        let f = fs::File::create(dir.join("s.png")).unwrap();
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
        make_pet(&bundled, "plum", &manifest("plum", "s.png"), Some(("s.png", PNG)));
        make_pet(&bundled, "perch", &manifest("perch", "s.png"), Some(("s.png", PNG)));
        make_pet(&bundled, "ember", &manifest("ember", "s.png"), Some(("s.png", PNG)));
        make_pet(&user, "mine", &manifest("mine", "s.webp"), Some(("s.webp", WEBP)));
        make_pet(&user, "perch-copy", &manifest("perch", "s.png"), Some(("s.png", PNG)));
        make_pet(&codex, "broken", "{", None);
        make_pet(&codex, "dewey", &manifest("dewey", "s.webp"), Some(("s.webp", WEBP)));
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

    #[test]
    fn resolves_with_fallbacks() {
        let pet = |id: &str| Pet {
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
        let png = t.path().join("a.png");
        fs::write(&png, PNG).unwrap();
        assert!(sprite_data_url(&png).unwrap().starts_with("data:image/png;base64,iVBORw0KGgo"));
        // A WebP saved with a .png extension still gets the WebP type.
        let misnamed = t.path().join("b.png");
        fs::write(&misnamed, WEBP).unwrap();
        assert!(sprite_data_url(&misnamed).unwrap().starts_with("data:image/webp;base64,UklGR"));
        let junk = t.path().join("c.webp");
        fs::write(&junk, b"hello").unwrap();
        assert!(sprite_data_url(&junk).is_err());
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
}
