/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// A line diff for failures: what was wanted against what was found, with unchanged runs folded.
use crate::rendering::Tone;

/// Unchanged lines kept around each change.
const Context: usize = 2;

/// The longest input a diff is computed for, in lines times lines. Past it the caller shows the
/// found value whole instead.
const MaxCells: usize = 4_000_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    OpSame,
    OpWanted,
    OpFound,
}

/// The diff of `wanted` against `found` as report lines: `-` only in wanted, `+` only in found.
/// `None` if the inputs are too large to diff.
pub fn render(wanted: &[&str], found: &[&str]) -> Option<Vec<(Tone, String)>> {
    let ops = compute(wanted, found)?;
    let mut keep = vec![false; ops.len()];
    for (i, (op, _)) in ops.iter().enumerate() {
        if *op != Op::OpSame {
            let from = i.saturating_sub(Context);
            let to = (i + Context + 1).min(ops.len());
            keep[from..to].iter_mut().for_each(|k| *k = true);
        }
    }
    let mut out = Vec::new();
    let mut skipped = 0;
    for (i, (op, line)) in ops.iter().enumerate() {
        if !keep[i] {
            skipped += 1;
            continue;
        }
        if skipped > 0 {
            let more = if skipped == 1 { "line" } else { "lines" };
            out.push((Tone::ToneDim, format!("  ... {skipped} unchanged {more}")));
            skipped = 0;
        }
        out.push(match op {
            Op::OpSame => (Tone::ToneNone, format!("  {line}")),
            Op::OpWanted => (Tone::ToneRemoved, format!("- {line}")),
            Op::OpFound => (Tone::ToneAdded, format!("+ {line}")),
        });
    }
    if skipped > 0 {
        let more = if skipped == 1 { "line" } else { "lines" };
        out.push((Tone::ToneDim, format!("  ... {skipped} unchanged {more}")));
    }
    Some(out)
}

/// Longest-common-subsequence diff.
fn compute(wanted: &[&str], found: &[&str]) -> Option<Vec<(Op, String)>> {
    let (n, m) = (wanted.len(), found.len());
    if n.checked_mul(m)? > MaxCells {
        return None;
    }
    // table[i][j]: length of the common subsequence of wanted[i..] and found[j..].
    let mut table = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i][j] = if wanted[i] == found[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let mut ops = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if wanted[i] == found[j] {
            ops.push((Op::OpSame, wanted[i].to_string()));
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            ops.push((Op::OpWanted, wanted[i].to_string()));
            i += 1;
        } else {
            ops.push((Op::OpFound, found[j].to_string()));
            j += 1;
        }
    }
    ops.extend(wanted[i..].iter().map(|l| (Op::OpWanted, l.to_string())));
    ops.extend(found[j..].iter().map(|l| (Op::OpFound, l.to_string())));
    Some(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ensure;

    fn text(lines: Vec<(Tone, String)>) -> Vec<String> {
        lines.into_iter().map(|(_, l)| l).collect()
    }

    #[test]
    fn equal_inputs_have_no_changes() {
        ensure(text(render(&["a", "b"], &["a", "b"]).unwrap()).len()).named("lines").is(1);
    }

    #[test]
    fn a_change_shows_both_sides() {
        let lines = render(&["a", "b", "c"], &["a", "x", "c"]).unwrap();
        ensure(text(lines)).is(vec!["  a", "- b", "+ x", "  c"]);
    }

    #[test]
    fn added_and_removed_lines_at_the_ends() {
        let lines = render(&["a"], &["a", "b"]).unwrap();
        ensure(text(lines)).is(vec!["  a", "+ b"]);
        let lines = render(&["a", "b"], &["b"]).unwrap();
        ensure(text(lines)).is(vec!["- a", "  b"]);
    }

    #[test]
    fn tones_follow_the_marker() {
        let lines = render(&["a"], &["b"]).unwrap();
        ensure(lines.iter().map(|(t, _)| *t).collect::<Vec<_>>())
            .is(vec![Tone::ToneRemoved, Tone::ToneAdded]);
    }

    #[test]
    fn long_unchanged_runs_are_folded_around_a_change() {
        let wanted: Vec<String> = (1..=20).map(|n| format!("l{n}")).collect();
        let mut found = wanted.clone();
        found[9] = "changed".to_string();
        let w: Vec<&str> = wanted.iter().map(String::as_str).collect();
        let f: Vec<&str> = found.iter().map(String::as_str).collect();
        ensure(text(render(&w, &f).unwrap())).is(vec![
            "  ... 7 unchanged lines",
            "  l8",
            "  l9",
            "- l10",
            "+ changed",
            "  l11",
            "  l12",
            "  ... 8 unchanged lines",
        ]);
    }

    #[test]
    fn a_single_skipped_line_is_said_in_the_singular() {
        let wanted = ["a", "b", "c", "d", "e", "f", "g"];
        let found = ["x", "b", "c", "d", "e", "f", "y"];
        let lines = text(render(&wanted, &found).unwrap());
        ensure(lines).contains("  ... 1 unchanged line".to_string());
    }

    #[test]
    fn inputs_too_large_to_diff_give_nothing() {
        let big: Vec<&str> = vec!["x"; 3000];
        ensure(render(&big, &big).is_none()).is_true();
    }
}
