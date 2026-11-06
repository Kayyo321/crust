/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Every check accepts what it should accept, and the checks chain.
use ensure::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[test]
fn equality_reads_like_a_sentence() {
    ensure(2 + 2).is(4);
    ensure(2 + 2).cannot_be(5);
    ensure("crust".to_string()).is("crust");
    ensure(vec![1, 2]).is(vec![1, 2]);
    ensure(3).is_one_of([1, 2, 3]);
    ensure(4).cannot_be_one_of([1, 2, 3]);
}

#[test]
fn order_checks_know_their_bounds() {
    ensure(5).is_above(4).is_below(6).is_at_least(5).is_at_most(5);
    ensure(5).is_between(5, 9);
    ensure(9).is_between(5, 9);
    ensure(0.5).is_above(0.25);
}

#[test]
fn bool_option_and_result_continue_the_chain() {
    ensure(1 < 2).is_true();
    ensure(2 < 1).is_false();
    ensure(Some(7)).is_some().is(7);
    ensure(None::<i32>).is_none();
    ensure(Ok::<_, String>(7)).is_ok().is_above(6);
    ensure(Err::<i32, _>("no")).is_err().is("no");
}

#[test]
fn text_checks() {
    ensure("crust").starts_with("cr").ends_with("st").contains("us").cannot_contain("x");
    ensure(String::from("crust")).contains('u').contains(String::from("rus"));
    ensure("").is_empty();
    ensure("crust").cannot_be_empty().has_len(5);
}

#[test]
fn collection_checks() {
    ensure(vec![1, 2, 3]).has_len(3).contains(2).cannot_contain(9).cannot_be_empty();
    ensure([1, 2, 3]).has_len(3).contains(3);
    ensure(&[1, 2][..]).contains(1);
    ensure(vec!["a".to_string()]).contains("a").cannot_contain("b");
    ensure(Vec::<i32>::new()).is_empty();
    ensure(HashSet::from([1])).has_len(1);
    ensure(BTreeSet::from([1, 2])).has_len(2);
    ensure(HashMap::from([(1, 2)])).has_len(1);
    ensure(BTreeMap::from([(1, 2)])).has_len(1);
}

#[test]
fn custom_tests_and_floats() {
    ensure(10).satisfies("is even", |n| n % 2 == 0);
    ensure(0.1 + 0.2).is_close_to(0.3, 1e-9);
}

#[test]
fn closures_that_panic_and_closures_that_do_not() {
    ensure(|| panic!("zero is not allowed")).panics().contains("zero");
    ensure(|| 6 * 7).does_not_panic().is(42);
}

#[test]
fn a_name_and_the_value_survive_the_chain() {
    let value = ensure(41 + 1).named("answer").is(42).into_value();
    ensure(value).is(42);
}
