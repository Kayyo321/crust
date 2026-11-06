/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::util::{crepe_error, crepe_result, hash_bytes, status};

/** `$CRUST_HOME`, or `~/.crepe`. */
pub fn crust_home() -> crepe_result<PathBuf> {
    if let Some(home) = std::env::var_os("CRUST_HOME").filter(|h| !h.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    match std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        Some(home) => Ok(PathBuf::from(home).join(".crepe")),
        None => Err(crepe_error::new("could not find the cache directory: set CRUST_HOME or HOME")),
    }
}

/** `$CRUST_HOME/git/<name>-<short hash of url>`. */
pub fn checkout_dir(name: &str, url: &str) -> crepe_result<PathBuf> {
    let short = &format!("{:016x}", hash_bytes(url.as_bytes()))[..8];
    Ok(crust_home()?.join("git").join(format!("{name}-{short}")))
}

fn git(dir: Option<&Path>, args: &[&str]) -> crepe_result<String> {
    let mut cmd = Command::new("git");
    if let Some(d) = dir {
        cmd.arg("-C").arg(d);
    }
    cmd.args(args);
    let output = cmd.output().map_err(|e| crepe_error::new(format!("could not run `git`: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crepe_error::new(format!(
            "`git {}` failed:\n{}",
            args.join(" "),
            stderr.trim_end()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/** Resolves a revision to a full commit hash in the clone at `dir`. */
fn resolve_rev(dir: &Path, rev: Option<&str>) -> Option<String> {
    let candidates: Vec<String> = match rev {
        // Remote-tracking names first: local branches in a cache clone go stale.
        Some(r) => vec![format!("origin/{r}"), r.to_string()],
        None => vec!["origin/HEAD".to_string(), "HEAD".to_string()],
    };
    candidates.iter().find_map(|c| {
        git(Some(dir), &["rev-parse", "--verify", "--quiet", &format!("{c}^{{commit}}")]).ok()
    })
}

fn fetch(dir: &Path, url: &str, rev: Option<&str>) -> crepe_result<()> {
    status("Fetching", url);
    git(Some(dir), &["fetch", "--quiet", "--tags", "origin"])?;
    // Fetching a bare commit hash works only when the server allows it; ignore failure.
    if let Some(r) = rev {
        let _ = git(Some(dir), &["fetch", "--quiet", "origin", r]);
    }
    Ok(())
}

/**
 * Clones or reuses the cache checkout of a git dependency and checks out a commit.
 *
 * With `locked`, that exact commit is used, fetching only if it is missing. Without it,
 * `rev` (or the remote's default branch) is resolved; `refresh` fetches first, which
 * `crepe update` uses to move to the newest matching commit.
 */
pub fn checkout(
    name: &str,
    url: &str,
    rev: Option<&str>,
    locked: Option<&str>,
    refresh: bool,
) -> crepe_result<(PathBuf, String)> {
    let dir = checkout_dir(name, url)?;
    let fresh_clone = !dir.join(".git").exists();
    if fresh_clone {
        status("Cloning", url);
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                crepe_error::new(format!("could not create `{}`: {e}", parent.display()))
            })?;
        }
        let target = dir.to_string_lossy().into_owned();
        git(None, &["clone", "--quiet", "--no-checkout", url, &target])?;
    }
    let commit = match locked {
        Some(c) => {
            if resolve_rev(&dir, Some(c)).is_none() {
                fetch(&dir, url, Some(c))?;
            }
            resolve_rev(&dir, Some(c))
        }
        None => {
            if refresh && !fresh_clone {
                fetch(&dir, url, rev)?;
            }
            let mut found = resolve_rev(&dir, rev);
            if found.is_none() && !refresh {
                fetch(&dir, url, rev)?;
                found = resolve_rev(&dir, rev);
            }
            found
        }
    };
    let want = locked.or(rev).unwrap_or("the default branch");
    let commit = commit.ok_or_else(|| {
        crepe_error::new(format!("could not find `{want}` in `{url}` (dependency `{name}`)"))
    })?;
    let head = git(Some(&dir), &["rev-parse", "--verify", "--quiet", "HEAD"]).ok();
    let clean = git(Some(&dir), &["status", "--porcelain"]).is_ok_and(|s| s.is_empty());
    if head.as_deref() != Some(commit.as_str()) || !clean {
        git(Some(&dir), &["checkout", "--quiet", "--force", "--detach", &commit])?;
    }
    Ok((dir, commit))
}
