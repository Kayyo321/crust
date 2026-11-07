/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// The shape of a failure report, checked as exact text. The reports are built against real
// files in a scratch folder, so the source lines in them are known.
use ensure::*;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ensure-rendering-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes `text` to a file in a scratch folder and gives its path as text.
fn file(name: &str, text: &str) -> String {
    let path = scratch(name).join("subject.rs");
    std::fs::write(&path, text).unwrap();
    path.display().to_string()
}

fn lines(text: &str) -> Vec<String> {
    text.lines().map(String::from).collect()
}

const Source: &str =
    "fn one() {}\nfn two() {}\nfn three() {\n    ensure(x).is(2);\n}\nfn five() {}\n";

#[test]
fn a_report_shows_the_failing_line_with_context_and_the_call_underlined() {
    let path = file("snippet", Source);
    let shown = report::error("ensure failed")
        .at(&path, 4, 15)
        .row("wanted", "exactly 2")
        .row("found", "1")
        .render_with(false);
    let expected = vec![
        "error: ensure failed".to_string(),
        format!(" --> {path}:4:15"),
        "  |".to_string(),
        "2 | fn two() {}".to_string(),
        "3 | fn three() {".to_string(),
        "4 |     ensure(x).is(2);".to_string(),
        "  |               ^^^^^ wanted exactly 2, found 1".to_string(),
        "5 | }".to_string(),
        "  |".to_string(),
        "  = wanted: exactly 2".to_string(),
        "  =  found: 1".to_string(),
    ];
    ensure(lines(&shown)).is(expected);
}

#[test]
fn a_report_without_a_readable_source_still_says_where() {
    let shown = report::error("ensure failed")
        .at("no/such/file.rs", 7, 3)
        .row("wanted", "a")
        .render_with(false);
    ensure(lines(&shown)).is(vec![
        "error: ensure failed".to_string(),
        " --> no/such/file.rs:7:3".to_string(),
        "  = wanted: a".to_string(),
    ]);
}

#[test]
fn line_numbers_of_any_width_stay_aligned() {
    let mut text = String::new();
    for n in 1..=101 {
        text.push_str(&format!("// line {n}\n"));
    }
    let path = file("wide", &text);
    let shown =
        report::error("e").at(&path, 99, 1).underline(2).row("wanted", "x").render_with(false);
    let lines = lines(&shown);
    ensure(lines[1].as_str()).is(format!("   --> {path}:99:1"));
    ensure(lines[3].as_str()).is(" 97 | // line 97");
    ensure(lines[5].as_str()).is(" 99 | // line 99");
    ensure(lines[6].as_str()).is("    | ^^ wanted x");
    ensure(lines[7].as_str()).is("100 | // line 100");
}

#[test]
fn tabs_in_the_source_do_not_shift_the_underline() {
    let path = file("tabs", "fn f() {\n\tensure(x).is(2);\n}\n");
    let shown = report::error("e").at(&path, 2, 12).row("wanted", "2").render_with(false);
    let lines = lines(&shown);
    ensure(lines[4].as_str()).is("2 |     ensure(x).is(2);");
    ensure(lines[5].as_str()).is("  |               ^^^^^ wanted 2");
}

#[test]
fn the_column_counts_characters_not_bytes() {
    let path = file("unicode", "let é = x.is(1);\n");
    let shown = report::error("e").at(&path, 1, 11).row("wanted", "1").render_with(false);
    let lines = lines(&shown);
    ensure(lines[3].as_str()).is("1 | let é = x.is(1);");
    ensure(lines[4].as_str()).is("  |           ^^^^^ wanted 1");
}

#[test]
fn a_call_that_continues_on_the_next_line_is_underlined_to_the_end_of_the_line() {
    let path = file("multi", "ensure(x).is(\n    2,\n);\n");
    let shown = report::error("e").at(&path, 1, 11).row("wanted", "2").render_with(false);
    ensure(lines(&shown)[3].as_str()).is("1 | ensure(x).is(");
    ensure(lines(&shown)[4].as_str()).is("  |           ^^^ wanted 2");
}

#[test]
fn parentheses_inside_a_string_do_not_end_the_underline_early() {
    let path = file("parens", "ensure(x).contains(\")\");\n");
    let shown = report::error("e").at(&path, 1, 11).row("wanted", "w").render_with(false);
    ensure(lines(&shown)[4].as_str()).is("  |           ^^^^^^^^^^^^^ wanted w");
}

#[test]
fn rows_are_right_aligned_and_blocks_sit_behind_the_gutter() {
    let shown = report::error("e")
        .at("nowhere.rs", 3, 1)
        .row("wanted", "a")
        .row("differs", "at character 2")
        .text_block("stderr", "first\nsecond")
        .help("try this")
        .render_with(false);
    ensure(lines(&shown)).is(vec![
        "error: e".to_string(),
        " --> nowhere.rs:3:1".to_string(),
        "  =  wanted: a".to_string(),
        "  = differs: at character 2".to_string(),
        "  |".to_string(),
        "  = stderr:".to_string(),
        "  |   first".to_string(),
        "  |   second".to_string(),
        "  = help: try this".to_string(),
    ]);
}

#[test]
fn a_long_block_is_folded() {
    let text: Vec<String> = (1..=45).map(|n| format!("line {n}")).collect();
    let shown = report::error("e").text_block("output", &text.join("\n")).render_with(false);
    let lines = lines(&shown);
    ensure(lines.len()).named("lines in the report").is(1 + 2 + 40 + 1);
    ensure(lines.last().unwrap().as_str()).is("  |   ... 5 more lines");
}

#[test]
fn a_report_without_a_place_has_no_arrow() {
    let shown = report::error("e").row("wanted", "a").render_with(false);
    ensure(lines(&shown)).is(vec!["error: e".to_string(), "  = wanted: a".to_string()]);
}

#[test]
fn an_explicit_label_replaces_the_wanted_and_found_label() {
    let path = file("label", Source);
    let shown =
        report::error("e").at(&path, 4, 15).label("custom").row("wanted", "w").render_with(false);
    ensure(lines(&shown)[6].as_str()).is("  |               ^^^^^ custom");
}

#[test]
fn a_long_value_sends_the_reader_to_the_rows_below() {
    let path = file("seebelow", Source);
    let shown =
        report::error("e").at(&path, 4, 15).row("wanted", "x".repeat(60)).render_with(false);
    ensure(lines(&shown)[6].as_str()).is("  |               ^^^^^ see below");
}

#[test]
fn color_wraps_the_parts_and_plain_text_survives() {
    let colored = report::error("e").row("wanted", "a").render_with(true);
    ensure(colored.as_str()).starts_with("\x1b[1;31merror\x1b[0m: ");
    ensure(plain(&colored)).is(report::error("e").row("wanted", "a").render_with(false));
}

#[test]
fn diff_lines_are_colored_by_what_they_say() {
    let diff = vec![
        (Tone::ToneNone, "  same".to_string()),
        (Tone::ToneRemoved, "- gone".to_string()),
        (Tone::ToneAdded, "+ new".to_string()),
    ];
    let colored = report::error("e").diff_block("d", diff).render_with(true);
    ensure(colored.as_str()).contains("\x1b[31m- gone\x1b[0m").contains("\x1b[32m+ new\x1b[0m");
}

// The one end-to-end check that a real failure carries its own source line.
#[test]
fn a_failed_check_shows_its_own_source_line_and_underlines_the_call() {
    let outcome = std::panic::catch_unwind(|| {
        ensure(3).named("exit code").is(0);
    });
    let message = match outcome {
        Err(payload) => payload.downcast_ref::<String>().cloned().unwrap_or_default(),
        Ok(()) => String::new(),
    };
    let message = plain(&message);
    let underline = message.lines().find(|l| l.contains('^')).unwrap_or("").to_string();
    ensure(message.as_str()).contains("ensure(3).named(\"exit code\").is(0);");
    ensure(underline.trim_start_matches([' ', '|'])).is("^^^^^ wanted exactly 0, found 3");
}
