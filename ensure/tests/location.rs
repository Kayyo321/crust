/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// A failed check reports the line of the test that called it, not a line inside the library.
// This is the only test in this file because it installs a process-wide panic hook.
use ensure::*;
use std::sync::Mutex;

static Seen: Mutex<Option<(String, u32)>> = Mutex::new(None);

#[test]
fn a_failure_points_at_the_calling_line() {
    std::panic::set_hook(Box::new(|info| {
        let place = info.location().map(|l| (l.file().to_string(), l.line()));
        *Seen.lock().unwrap() = place;
    }));
    let line = line!() + 1;
    let outcome = std::panic::catch_unwind(|| ensure(1).is(2));
    let _ = std::panic::take_hook();

    ensure(outcome.is_err()).is_true();
    let (file, reported) = Seen.lock().unwrap().take().unwrap();
    ensure(file).ends_with("location.rs");
    ensure(reported).is(line);
}
