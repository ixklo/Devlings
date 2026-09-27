//! The app is called Devlings (design `docs/specs/2026-09-27-devlings-v1.1.md`). "Perch" is now only the teal
//! bird, one of the bundled pets. These checks keep the old app name out of everything the backend can show or say:
//! string literals in the app's Rust code (tests excluded) and the Tauri config.

use std::{fs, path::Path};

/// Every string literal that may still contain "Perch", as (file, exact literal). Each one is the bird, not the app.
const ALLOWED: &[(&str, &str)] = &[
    // The name every install starts its pet with: the default pet is the bird called Perch.
    ("store.rs", "Perch"),
];

/// String literals in Rust source, with their line numbers. Comments, char literals and lifetimes are skipped; raw
/// and byte strings count.
fn string_literals(src: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let prev_ident = i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                line += usize::from(chars[i] == '\n');
                i += 1;
            }
            i += 2;
        } else if c == '\'' {
            // A char literal ('"', '\'', 'x') or a lifetime ('a).
            if next == Some('\\') {
                i += 3;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if chars.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
        } else if (c == 'r' || (c == 'b' && next == Some('r'))) && !prev_ident && {
            let r = if c == 'b' { i + 1 } else { i };
            matches!(chars.get(r + 1), Some('"') | Some('#'))
        } {
            let mut j = if c == 'b' { i + 2 } else { i + 1 };
            let mut hashes = 0;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) != Some(&'"') {
                i = j;
                continue;
            }
            let (start_line, mut text) = (line, String::new());
            j += 1;
            while j < chars.len() {
                if chars[j] == '"' && (1..=hashes).all(|k| chars.get(j + k) == Some(&'#')) {
                    break;
                }
                line += usize::from(chars[j] == '\n');
                text.push(chars[j]);
                j += 1;
            }
            out.push((start_line, text));
            i = j + 1 + hashes;
        } else if c == '"' {
            let (start_line, mut text) = (line, String::new());
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    text.push(chars[i]);
                    i += 1;
                }
                if let Some(&ch) = chars.get(i) {
                    line += usize::from(ch == '\n');
                    text.push(ch);
                }
                i += 1;
            }
            out.push((start_line, text));
            i += 1;
        } else {
            i += 1;
        }
    }
    out
}

/// The part of a source file before its unit tests (`#[cfg(test)]` followed by `mod tests`).
fn without_tests(src: &str) -> &str {
    src.find("#[cfg(test)]\nmod tests")
        .or_else(|| src.find("#[cfg(test)]\r\nmod tests"))
        .map_or(src, |at| &src[..at])
}

#[test]
fn the_scanner_finds_string_literals_only() {
    let src = "// \"Perch\" in a comment\nlet a = \"one\"; /* \"two\" */ let q = '\"'; let s = '\\''; fn f<'a>(x: &'a str) {}\n\
               let b = r#\"raw \"three\"\"#; let c = b\"four\"; let d = \"esc \\\" five\";\n\
               let e = \"multi\nline\";";
    let found: Vec<(usize, String)> = string_literals(src);
    assert_eq!(
        found,
        vec![
            (2, "one".to_string()),
            (3, "raw \"three\"".to_string()),
            (3, "four".to_string()),
            (3, "esc \\\" five".to_string()),
            (4, "multi\nline".to_string())
        ]
    );
    assert_eq!(without_tests("fn a() {}\n#[cfg(test)]\nmod tests {}"), "fn a() {}\n");
}

#[test]
fn rust_strings_never_call_the_app_perch() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    let mut files = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect::<Vec<_>>();
    files.sort();
    assert!(files.len() > 10, "expected the app's modules in {}", dir.display());
    for path in files.iter().filter(|p| p.extension().is_some_and(|e| e == "rs")) {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let src = fs::read_to_string(path).unwrap();
        for (line, text) in string_literals(without_tests(&src)) {
            let old_name = text.contains("Perch") || text.contains("yeetstick") || text.contains("/perch/");
            if old_name && !ALLOWED.contains(&(name.as_str(), text.as_str())) {
                offenders.push(format!("{name}:{line}: {text:?}"));
            }
        }
    }
    assert!(offenders.is_empty(), "these strings still call the app Perch:\n{}", offenders.join("\n"));
}

#[test]
fn every_allowed_string_is_still_there() {
    // A stale allow-list entry would hide nothing, but it would also stop documenting anything real.
    for (file, text) in ALLOWED {
        let src = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file)).unwrap();
        assert!(
            string_literals(without_tests(&src)).iter().any(|(_, t)| t == text),
            "{file} no longer has {text:?}; drop it from ALLOWED"
        );
    }
}

#[test]
fn tauri_config_names_the_app_devlings() {
    let raw = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json")).unwrap();
    let conf: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(conf["productName"], "Devlings");
    assert_eq!(conf["mainBinaryName"], "devlings");
    // Kept on purpose: it names the app-data folder, so settings carry over from Perch without a migration.
    assert_eq!(conf["identifier"], "io.github.perchpet.perch");
    for window in conf["app"]["windows"].as_array().unwrap() {
        assert_eq!(window["title"], "Devlings", "{window}");
    }
    assert_eq!(
        conf["plugins"]["updater"]["endpoints"],
        serde_json::json!(["https://github.com/ixklo/devlings/releases/latest/download/latest.json"])
    );
    // The identifier is the only place the old name (or the repository's old owner) may appear.
    let rest = raw.replace("io.github.perchpet.perch", "").to_lowercase();
    assert!(!rest.contains("perch") && !rest.contains("yeetstick"), "{raw}");
}
