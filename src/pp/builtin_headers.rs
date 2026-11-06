/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/

/// Freestanding headers that come with the compiler rather than with libc.
/// They live in lib/include/ and are embedded in the binary.
pub fn get(name: &str) -> Option<&'static str> {
    Some(match name {
        "stddef.h" => include_str!("../../lib/include/stddef.h"),
        "stdarg.h" => include_str!("../../lib/include/stdarg.h"),
        "stdbool.h" => include_str!("../../lib/include/stdbool.h"),
        "stdalign.h" => include_str!("../../lib/include/stdalign.h"),
        "stdnoreturn.h" => include_str!("../../lib/include/stdnoreturn.h"),
        "iso646.h" => include_str!("../../lib/include/iso646.h"),
        "float.h" => include_str!("../../lib/include/float.h"),
        _ => return None,
    })
}
