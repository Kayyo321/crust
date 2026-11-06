/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// The safety analyses (docs/MEMORY_SAFETY.md, section 3). They run on CrIR
// and report diagnostics; drop elaboration also rewrites the body.
pub mod bitset;
pub mod borrow;
pub mod init;

use crate::crir::*;
use crate::sema::tir::program;

/// Runs every analysis on one function body, in order.
/// With `report` off (`--unchecked`), only drop elaboration runs.
pub fn check_body(b: &mut body, prog: &program, report: bool) {
    init::mark_zero_init(b, prog);
    let md = init::move_data::build(b, prog);
    let states = init::analyze(b, prog, &md);
    if report {
        init::report(b, prog, &md, &states);
    }
    init::elaborate_drops(b, prog, &md, &states);
    if report {
        borrow::check(b, prog);
    }
}

/// A readable name for a place, for diagnostics.
pub fn describe_place(b: &body, p: &place, prog: &program) -> String {
    let mut s = match p.base {
        PlaceBaseLocal(l) => match b.locals[l as usize].name {
            Some(n) => n.to_string(),
            None if l == ReturnSlot => "the return value".into(),
            None => "a temporary".into(),
        },
        PlaceBaseGlobal(g) => prog.globals[g as usize].name.to_string(),
        PlaceBaseString(_) => "a string literal".into(),
    };
    for pr in &p.proj {
        match pr {
            ProjDeref => s = format!("*{s}"),
            ProjField(r, i) => {
                let n = crate::sema::types::with_record(*r, |rd| rd.fields[*i as usize].name);
                s = format!(
                    "{s}.{}",
                    n.map(|n| n.to_string()).unwrap_or_else(|| "(anonymous)".into())
                );
            }
            ProjIndex(_) => s = format!("{s}[..]"),
            ProjConstIndex(i) => s = format!("{s}[{i}]"),
        }
    }
    s
}
