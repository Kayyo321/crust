/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::util::{crepe_error, crepe_result, fnv_hasher, hex64};

/**
 * First line of a module state file. The format has no comment syntax, so it carries no
 * file header (CODE_STYLE.md, section 1: machine-written files are exempt).
 */
const StateMagic: &str = "crepe-module-state 1";

/** The magic bytes that start every `.crmi` interface file. */
const InterfaceMagic: &[u8; 4] = b"CRMI";

/** What a module was compiled from. A module is fresh when all of it still matches (D-030). */
#[derive(Clone, Debug, PartialEq)]
pub struct module_state {
    /** Hash of the source file and the quoted includes it reaches. */
    pub source_hash: u64,
    /** Hash of the crust command line, profile, and compiler identity. */
    pub flags_hash: u64,
    /** `(import key, interface hash)`, sorted by key. */
    pub imports: Vec<(String, u64)>,
}

pub fn format_state(state: &module_state) -> String {
    let mut out = format!(
        "{StateMagic}\nsource {}\nflags {}\n",
        hex64(state.source_hash),
        hex64(state.flags_hash)
    );
    for (key, hash) in &state.imports {
        out.push_str(&format!("import {key} {}\n", hex64(*hash)));
    }
    out
}

/** Parses a state file. Anything unreadable counts as no state, which forces a rebuild. */
pub fn parse_state(text: &str) -> Option<module_state> {
    let mut lines = text.lines();
    if lines.next()? != StateMagic {
        return None;
    }
    let mut source_hash = None;
    let mut flags_hash = None;
    let mut imports = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.split(' ').collect();
        match fields.as_slice() {
            ["source", h] => source_hash = Some(u64::from_str_radix(h, 16).ok()?),
            ["flags", h] => flags_hash = Some(u64::from_str_radix(h, 16).ok()?),
            ["import", key, h] => imports.push((key.to_string(), u64::from_str_radix(h, 16).ok()?)),
            _ => return None,
        }
    }
    Some(module_state { source_hash: source_hash?, flags_hash: flags_hash?, imports })
}

pub fn load_state(path: &Path) -> Option<module_state> {
    parse_state(&fs::read_to_string(path).ok()?)
}

/** Why a module must be recompiled, or `None` when it is fresh. */
pub fn stale_reason(
    previous: Option<&module_state>,
    current: &module_state,
    outputs_exist: bool,
) -> Option<String> {
    let Some(previous) = previous else {
        return Some("not compiled yet".to_string());
    };
    if !outputs_exist {
        return Some("outputs are missing".to_string());
    }
    if previous.source_hash != current.source_hash {
        return Some("source changed".to_string());
    }
    if previous.flags_hash != current.flags_hash {
        return Some("flags changed".to_string());
    }
    if previous.imports != current.imports {
        let changed: Vec<&str> = current
            .imports
            .iter()
            .filter(|entry| !previous.imports.contains(entry))
            .map(|(key, _)| key.as_str())
            .collect();
        if changed.is_empty() {
            return Some("imports changed".to_string());
        }
        return Some(format!("interface of {} changed", changed.join(", ")));
    }
    None
}

/**
 * Reads the interface hash from a `.crmi` header: `CRMI`, a u32 LE format version, then
 * the u64 LE interface hash.
 */
pub fn read_interface_hash(path: &Path) -> crepe_result<u64> {
    let bytes = fs::read(path).map_err(|e| {
        crepe_error::new(format!("could not read interface `{}`: {e}", path.display()))
    })?;
    parse_interface_hash(&bytes).ok_or_else(|| {
        crepe_error::new(format!("`{}` is not a valid .crmi interface file", path.display()))
    })
}

pub fn parse_interface_hash(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 16 || &bytes[..4] != InterfaceMagic {
        return None;
    }
    let hash: [u8; 8] = bytes[8..16].try_into().ok()?;
    Some(u64::from_le_bytes(hash))
}

/**
 * Hashes a source file plus every `#include "..."` it reaches that exists next to the
 * including file, so editing a `.hr` header recompiles the modules that include it.
 */
pub fn hash_source(path: &Path, includes_of: &dyn Fn(&str) -> Vec<String>) -> crepe_result<u64> {
    let mut hasher = fnv_hasher::new();
    let mut seen = BTreeSet::new();
    let mut pending = vec![path.to_path_buf()];
    while let Some(file) = pending.pop() {
        if !seen.insert(file.clone()) {
            continue;
        }
        let bytes = fs::read(&file)
            .map_err(|e| crepe_error::new(format!("could not read `{}`: {e}", file.display())))?;
        hasher.update(&bytes);
        hasher.update(&[0xff]);
        let text = String::from_utf8_lossy(&bytes);
        let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut found: Vec<PathBuf> =
            includes_of(&text).iter().map(|inc| dir.join(inc)).filter(|p| p.is_file()).collect();
        found.reverse();
        pending.extend(found);
    }
    Ok(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(source: u64, flags: u64, imports: &[(&str, u64)]) -> module_state {
        module_state {
            source_hash: source,
            flags_hash: flags,
            imports: imports.iter().map(|(k, h)| (k.to_string(), *h)).collect(),
        }
    }

    #[test]
    fn state_round_trips() {
        let s = state(1, 2, &[("geometry", 3), ("mathx/core", 0xdead_beef)]);
        assert_eq!(parse_state(&format_state(&s)), Some(s));
        assert_eq!(parse_state("garbage"), None);
        assert_eq!(parse_state("crepe-module-state 1\nsource zz\n"), None);
    }

    #[test]
    fn fresh_when_nothing_changed() {
        let s = state(1, 2, &[("geometry", 3)]);
        assert_eq!(stale_reason(Some(&s), &s.clone(), true), None);
    }

    #[test]
    fn rebuilds_on_source_flags_or_missing_outputs() {
        let old = state(1, 2, &[]);
        assert!(stale_reason(None, &old, true).is_some());
        assert_eq!(stale_reason(Some(&old), &old, false).as_deref(), Some("outputs are missing"));
        assert_eq!(
            stale_reason(Some(&old), &state(9, 2, &[]), true).as_deref(),
            Some("source changed")
        );
        assert_eq!(
            stale_reason(Some(&old), &state(1, 9, &[]), true).as_deref(),
            Some("flags changed")
        );
    }

    #[test]
    fn rebuilds_importer_only_when_an_interface_hash_changes() {
        let old = state(1, 2, &[("geometry", 3), ("util", 4)]);
        // A body-only change in `geometry` leaves its interface hash at 3: fresh.
        assert_eq!(
            stale_reason(Some(&old), &state(1, 2, &[("geometry", 3), ("util", 4)]), true),
            None
        );
        let changed = stale_reason(Some(&old), &state(1, 2, &[("geometry", 7), ("util", 4)]), true);
        assert_eq!(changed.as_deref(), Some("interface of geometry changed"));
        let dropped = stale_reason(Some(&old), &state(1, 2, &[("geometry", 3)]), true);
        assert_eq!(dropped.as_deref(), Some("imports changed"));
    }

    #[test]
    fn reads_interface_hash_header() {
        let mut bytes = b"CRMI".to_vec();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0x0123_4567_89ab_cdefu64.to_le_bytes());
        bytes.extend_from_slice(b"payload");
        assert_eq!(parse_interface_hash(&bytes), Some(0x0123_4567_89ab_cdef));
        assert_eq!(parse_interface_hash(b"CRMX\x01\0\0\0\0\0\0\0\0\0\0\0"), None);
        assert_eq!(parse_interface_hash(b"CRMI"), None);
    }
}
