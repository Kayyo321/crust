/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Every check refuses what it should refuse, and says why. The checks test themselves.
use ensure::*;

#[test]
fn a_failure_shows_what_was_wanted_and_what_was_found() {
    ensure(|| ensure(3).is(5))
        .panics()
        .contains("ensure failed\n")
        .contains("wanted: exactly 5")
        .contains("found: 3");
}

#[test]
fn a_name_heads_the_failure() {
    ensure(|| ensure(3).named("exit code").is(0)).panics().contains("ensure failed: exit code");
}

#[test]
fn equality_failures() {
    ensure(|| ensure(5).cannot_be(5)).panics().contains("wanted: anything but 5");
    ensure(|| ensure(9).is_one_of([1, 2])).panics().contains("wanted: one of [1, 2]");
    ensure(|| ensure(1).cannot_be_one_of([1, 2])).panics().contains("wanted: none of [1, 2]");
    ensure(|| ensure(7).satisfies("is even", |n| n % 2 == 0))
        .panics()
        .contains("wanted: a value that is even");
    ensure(|| ensure(1.0).is_close_to(2.0, 0.5)).panics().contains("give or take 0.5");
}

#[test]
fn nan_is_close_to_nothing() {
    ensure(|| ensure(f64::NAN).is_close_to(f64::NAN, 1.0)).panics().contains("wanted:");
}

#[test]
fn order_failures() {
    ensure(|| ensure(5).is_above(5)).panics().contains("wanted: above 5");
    ensure(|| ensure(5).is_below(5)).panics().contains("wanted: below 5");
    ensure(|| ensure(4).is_at_least(5)).panics().contains("wanted: at least 5");
    ensure(|| ensure(6).is_at_most(5)).panics().contains("wanted: at most 5");
    ensure(|| ensure(10).is_between(1, 9)).panics().contains("wanted: from 1 to 9");
}

#[test]
fn shape_failures() {
    ensure(|| ensure(false).is_true()).panics().contains("wanted: true");
    ensure(|| ensure(true).is_false()).panics().contains("wanted: false");
    ensure(|| ensure(None::<i32>).is_some()).panics().contains("wanted: a value (Some)");
    ensure(|| ensure(Some(1)).is_none()).panics().contains("found: Some(1)");
    ensure(|| ensure(Err::<i32, _>("bad")).is_ok()).panics().contains("found: Err(\"bad\")");
    ensure(|| ensure(Ok::<_, String>(1)).is_err()).panics().contains("found: Ok(1)");
}

#[test]
fn text_and_collection_failures() {
    ensure(|| ensure("crust").starts_with("rs")).panics().contains("text starting with \"rs\"");
    ensure(|| ensure("crust").ends_with("cr")).panics().contains("text ending with \"cr\"");
    ensure(|| ensure("crust").contains("zz")).panics().contains("wanted: to contain \"zz\"");
    ensure(|| ensure("crust").cannot_contain("us")).panics().contains("not to contain \"us\"");
    ensure(|| ensure(vec![1, 2]).has_len(3)).panics().contains("a length of 2: [1, 2]");
    ensure(|| ensure(vec![1]).is_empty()).panics().contains("nothing in it");
    ensure(|| ensure(Vec::<i32>::new()).cannot_be_empty()).panics().contains("something in it");
    ensure(|| ensure(vec![1, 2]).contains(9)).panics().contains("wanted: to contain 9");
}

#[test]
fn closure_failures() {
    ensure(|| ensure(|| 1).panics()).panics().contains("found: it returned normally");
    ensure(|| ensure(|| panic!("boom")).does_not_panic()).panics().contains("a panic: \"boom\"");
}
