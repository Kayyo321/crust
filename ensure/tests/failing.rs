/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Every check refuses what it should refuse, and says why. The checks test themselves.
//
// A failure report echoes the source line of the failed call. A test that checks the message
// would match its own expected text there, so every message here is taken `without_source`.
use ensure::*;

/// The report that `check` panics with, with no source lines in it.
fn failure<R>(check: impl FnOnce() -> R) -> subject<String> {
    without_source(|| ensure(check).panics())
}

/// The rows of a report (the `= name: value` lines), without the `=`.
fn rows(report: &str) -> Vec<String> {
    report.lines().filter_map(|l| l.trim_start().strip_prefix("= ")).map(String::from).collect()
}

#[test]
fn a_failure_is_an_error_with_a_place_and_rows() {
    let report = failure(|| ensure(3).is(5)).into_value();
    let lines: Vec<&str> = report.lines().collect();
    ensure(lines[0]).is("error: ensure failed");
    ensure(lines[1].trim_start()).starts_with("--> ").contains("tests/failing.rs:");
    ensure(rows(&report)).is(vec!["wanted: exactly 5", " found: 3", "  type: i32"]);
}

#[test]
fn a_name_heads_the_failure() {
    failure(|| ensure(3).named("exit code").is(0)).starts_with("error: ensure failed: exit code\n");
}

#[test]
fn the_report_lists_the_checks_that_passed_before_the_failure() {
    let report = failure(|| ensure(Some(7)).is_some().is_above(5).is_below(6)).into_value();
    ensure(rows(&report)).contains(" after: .is_some().is_above()");
    let report = failure(|| ensure(3).is(5)).into_value();
    ensure(rows(&report).iter().any(|r| r.contains("after"))).named("an 'after' row").is_false();
}

#[test]
fn equality_failures() {
    failure(|| ensure(5).cannot_be(5)).contains("wanted: anything but 5");
    failure(|| ensure(9).is_one_of([1, 2])).contains("wanted: one of [1, 2]");
    failure(|| ensure(1).cannot_be_one_of([1, 2])).contains("wanted: none of [1, 2]");
    failure(|| ensure(7).satisfies("is even", |n| n % 2 == 0))
        .contains("wanted: a value that is even");
    failure(|| ensure(1.0).is_close_to(2.0, 0.5))
        .contains("give or take 0.5")
        .contains("off by: 1.0");
}

#[test]
fn nan_is_close_to_nothing() {
    failure(|| ensure(f64::NAN).is_close_to(f64::NAN, 1.0)).contains("wanted:");
}

#[test]
fn order_failures() {
    failure(|| ensure(5).is_above(5)).contains("wanted: above 5");
    failure(|| ensure(5).is_below(5)).contains("wanted: below 5");
    failure(|| ensure(4).is_at_least(5)).contains("wanted: at least 5");
    failure(|| ensure(6).is_at_most(5)).contains("wanted: at most 5");
    failure(|| ensure(10).is_between(1, 9)).contains("wanted: from 1 to 9");
}

#[test]
fn shape_failures() {
    failure(|| ensure(false).is_true()).contains("wanted: true");
    failure(|| ensure(true).is_false()).contains("wanted: false");
    failure(|| ensure(None::<i32>).is_some()).contains("wanted: a value (Some)");
    failure(|| ensure(Some(1)).is_none()).contains("found: Some(1)");
    failure(|| ensure(Err::<i32, _>("bad")).is_ok()).contains("found: Err(\"bad\")");
    failure(|| ensure(Ok::<_, String>(1)).is_err()).contains("found: Ok(1)");
}

#[test]
fn text_and_collection_failures() {
    failure(|| ensure("crust").starts_with("rs")).contains("wanted: text starting with \"rs\"");
    failure(|| ensure("crust").ends_with("cr")).contains("wanted: text ending with \"cr\"");
    failure(|| ensure("crust").contains("zz")).contains("wanted: to contain \"zz\"");
    failure(|| ensure("crust").cannot_contain("us")).contains("wanted: not to contain \"us\"");
    failure(|| ensure(vec![1, 2]).has_len(3))
        .contains("wanted: a length of 3")
        .contains("length: 2")
        .contains("found: [1, 2]");
    failure(|| ensure(vec![1]).is_empty()).contains("wanted: nothing in it");
    failure(|| ensure(Vec::<i32>::new()).cannot_be_empty()).contains("wanted: something in it");
    failure(|| ensure(vec![1, 2]).contains(9))
        .contains("wanted: to contain 9")
        .contains("found: [1, 2]")
        .contains("type: Vec<i32>");
}

/// A type with no `Debug`.
struct opaque;

#[test]
fn failures_on_a_value_with_no_debug_still_report() {
    failure(|| ensure(None::<opaque>).is_some())
        .contains("wanted: a value (Some)")
        .contains("found: None");
    failure(|| ensure(Err::<opaque, _>("bad")).is_ok()).contains("found: Err(\"bad\")");
}

#[test]
fn picking_an_item_that_is_not_there() {
    failure(|| ensure(Vec::<i32>::new()).first())
        .contains("wanted: a first item")
        .contains("length: 0");
    failure(|| ensure(Vec::<i32>::new()).last()).contains("wanted: a last item");
    failure(|| ensure(vec![1, 2]).item(5))
        .contains("wanted: an item at index 5")
        .contains("length: 2")
        .contains("found: [1, 2]");
}

#[test]
fn a_check_on_a_picked_item_remembers_the_pick() {
    let report = failure(|| ensure(vec![1, 2]).item(1).is(9)).into_value();
    ensure(report).contains("after: .item()").contains("wanted: exactly 9").contains("found: 2");
}

#[test]
fn closure_failures() {
    failure(|| ensure(|| 1).panics()).contains("found: it returned normally");
    failure(|| ensure(|| panic!("boom")).does_not_panic())
        .contains("found: a panic: \"boom\"")
        .contains("= panic message:");
}

#[test]
fn a_closure_subject_has_no_type_row() {
    let report = failure(|| ensure(|| 1).panics()).into_value();
    ensure(report).cannot_contain("type:");
}

#[test]
fn text_that_differs_in_one_line_says_where() {
    let report = failure(|| ensure("crust build").is("crust built")).into_value();
    ensure(rows(&report)).contains("differs: at character 11: wanted 't', found 'd'");
    let report = failure(|| ensure("crust").is("crust built")).into_value();
    ensure(rows(&report)).contains("differs: found ends at character 6: wanted ' ' next");
    let report = failure(|| ensure("crust built").is("crust")).into_value();
    ensure(rows(&report)).contains("differs: wanted ends at character 6: found ' ' next");
}

#[test]
fn text_that_differs_only_in_case_or_whitespace_gets_a_hint() {
    failure(|| ensure("Crust").is("crust")).contains("help: the values differ only in letter case");
    failure(|| ensure(" crust\n").is("crust"))
        .contains("help: the values differ only in surrounding whitespace");
    failure(|| ensure("Crust says hi").contains("crust"))
        .contains("help: it is there when letter case is ignored");
}

#[test]
fn long_text_is_shown_as_a_block_and_a_diff() {
    let wanted = "one\ntwo\nthree\nfour\nfive\nsix\nseven";
    let got = "one\ntwo\n3\nfour\nfive\nsix";
    let report = failure(|| ensure(got).is(wanted)).into_value();
    ensure(report)
        .contains("found: text of 6 lines, 23 bytes")
        .contains("= difference (- wanted, + found):");
    let diff: Vec<String> = failure(|| ensure(got).is(wanted))
        .into_value()
        .lines()
        .filter(|l| l.contains("| ") && (l.contains("- ") || l.contains("+ ")))
        .map(|l| l.trim().to_string())
        .collect();
    ensure(diff).is(vec!["|   - three", "|   + 3", "|   - seven"]);
}

#[test]
fn lists_that_differ_show_a_diff_of_their_items() {
    let report = failure(|| ensure(vec![1, 2, 4]).is(vec![1, 2, 3])).into_value();
    ensure(report)
        .contains("= difference (- wanted, + found):")
        .contains("-     3,")
        .contains("+     4,");
}

#[test]
fn a_trailing_newline_is_the_only_difference() {
    failure(|| ensure("a\nb\n").is("a\nb"))
        .contains("help: the lines are the same; only a trailing newline or line ending differs");
}

#[test]
fn a_panic_message_with_colors_comes_back_plain() {
    ensure(|| panic!("\x1b[1;31mred\x1b[0m")).panics().is("red");
}
