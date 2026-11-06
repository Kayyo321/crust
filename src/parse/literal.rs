/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/

/// Encoding prefix of a character constant or string literal.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Encoding {
    EncodingPlain,
    EncodingUtf8,
    EncodingUtf16,
    EncodingUtf32,
    EncodingWide,
}
pub use Encoding::*;

impl Encoding {
    /// Size in bytes of one code unit.
    pub fn unit_size(self) -> u64 {
        match self {
            EncodingPlain | EncodingUtf8 => 1,
            EncodingUtf16 => 2,
            EncodingUtf32 | EncodingWide => 4,
        }
    }
}

fn split_prefix(s: &str) -> (Encoding, &str) {
    if let Some(r) = s.strip_prefix("u8") {
        (EncodingUtf8, r)
    } else if let Some(r) = s.strip_prefix('u') {
        (EncodingUtf16, r)
    } else if let Some(r) = s.strip_prefix('U') {
        (EncodingUtf32, r)
    } else if let Some(r) = s.strip_prefix('L') {
        (EncodingWide, r)
    } else {
        (EncodingPlain, s)
    }
}

/// Decodes the body of a quoted literal into code points. Escapes that name
/// a code unit directly (octal and hex) are returned as-is with `raw` set.
fn decode_body(body: &str) -> Result<Vec<(u32, bool)>, String> {
    let mut out = Vec::new();
    let b: Vec<char> = body.chars().collect();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c != '\\' {
            out.push((c as u32, false));
            i += 1;
            continue;
        }
        i += 1;
        let Some(&e) = b.get(i) else { return Err("trailing backslash".into()) };
        i += 1;
        let v = match e {
            'n' => 10,
            't' => 9,
            'r' => 13,
            'a' => 7,
            'b' => 8,
            'f' => 12,
            'v' => 11,
            'e' => 27,
            '\\' | '\'' | '"' | '?' => e as u32,
            '0'..='7' => {
                let mut v = e.to_digit(8).unwrap();
                let mut n = 1;
                while n < 3 && b.get(i).is_some_and(|d| d.is_digit(8)) {
                    v = v * 8 + b[i].to_digit(8).unwrap();
                    i += 1;
                    n += 1;
                }
                out.push((v, true));
                continue;
            }
            'x' => {
                let mut v: u64 = 0;
                let start = i;
                while b.get(i).is_some_and(|d| d.is_ascii_hexdigit()) {
                    v = v.wrapping_mul(16) + b[i].to_digit(16).unwrap() as u64;
                    i += 1;
                }
                if i == start {
                    return Err("\\x used with no following hex digits".into());
                }
                out.push((v as u32, true));
                continue;
            }
            'u' | 'U' => {
                let n = if e == 'u' { 4 } else { 8 };
                let hex: String = b.iter().skip(i).take(n).collect();
                if hex.len() != n || !hex.chars().all(|d| d.is_ascii_hexdigit()) {
                    return Err("incomplete universal character name".into());
                }
                i += n;
                u32::from_str_radix(&hex, 16).unwrap()
            }
            other => return Err(format!("unknown escape sequence `\\{other}`")),
        };
        out.push((v, false));
    }
    Ok(out)
}

/// The value of a character constant, as `int` (or the prefixed type).
pub fn char_value(spelling: &str) -> Option<i64> {
    char_literal(spelling).ok().map(|(v, _)| v)
}

pub fn char_literal(spelling: &str) -> Result<(i64, Encoding), String> {
    let (enc, rest) = split_prefix(spelling);
    let body = &rest[1..rest.len() - 1];
    let cps = decode_body(body)?;
    if cps.is_empty() {
        return Err("empty character constant".into());
    }
    match enc {
        EncodingPlain => {
            // Multi-character constants follow the GCC/Clang convention.
            let mut bytes = Vec::new();
            for (cp, raw) in &cps {
                if *raw {
                    bytes.push(*cp as u8);
                } else {
                    let mut buf = [0u8; 4];
                    let ch = char::from_u32(*cp).unwrap_or('?');
                    bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
            }
            if bytes.len() == 1 {
                return Ok((bytes[0] as i8 as i64, enc));
            }
            let mut v: i64 = 0;
            for b in bytes {
                v = (v << 8) | b as i64;
            }
            Ok((v as i32 as i64, enc))
        }
        EncodingUtf8 => Ok((cps[0].0 as u8 as i64, enc)),
        EncodingUtf16 => Ok((cps[0].0 as u16 as i64, enc)),
        EncodingUtf32 => Ok((cps[0].0 as i64, enc)),
        EncodingWide => Ok((cps[0].0 as i32 as i64, enc)),
    }
}

/// Decodes adjacent string literals into code units, without the terminator.
pub fn string_literal(pieces: &[&str]) -> Result<(Vec<u32>, Encoding), String> {
    let mut enc = EncodingPlain;
    for p in pieces {
        let (e, _) = split_prefix(p);
        if e != EncodingPlain {
            if enc != EncodingPlain && enc != e {
                return Err("concatenated string literals have different encoding prefixes".into());
            }
            enc = e;
        }
    }
    let mut units = Vec::new();
    for p in pieces {
        let (_, rest) = split_prefix(p);
        let body = &rest[1..rest.len() - 1];
        for (cp, raw) in decode_body(body)? {
            if raw {
                units.push(cp);
                continue;
            }
            match enc {
                EncodingPlain | EncodingUtf8 => {
                    let mut buf = [0u8; 4];
                    let ch = char::from_u32(cp).unwrap_or('\u{fffd}');
                    units.extend(ch.encode_utf8(&mut buf).bytes().map(|b| b as u32));
                }
                EncodingUtf16 => {
                    let mut buf = [0u16; 2];
                    let ch = char::from_u32(cp).unwrap_or('\u{fffd}');
                    units.extend(ch.encode_utf16(&mut buf).iter().map(|u| *u as u32));
                }
                EncodingUtf32 | EncodingWide => units.push(cp),
            }
        }
    }
    Ok((units, enc))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct int_suffix {
    pub unsigned: bool,
    /// 0 none, 1 `l`, 2 `ll`
    pub long: u8,
    pub decimal: bool,
}

pub fn int_literal(spelling: &str) -> Result<(u128, int_suffix), String> {
    let s: String = spelling.chars().filter(|c| *c != '\'').collect();
    let lower = s.to_ascii_lowercase();
    let (radix, digits_start) = if lower.starts_with("0x") {
        (16, 2)
    } else if lower.starts_with("0b") {
        (2, 2)
    } else if lower.starts_with('0') && lower.len() > 1 {
        (8, 1)
    } else {
        (10, 0)
    };
    let body = &lower[digits_start..];
    let end = body.find(|c: char| !c.is_digit(radix)).unwrap_or(body.len());
    let (digits, suffix) = body.split_at(end);
    if digits.is_empty() && radix != 8 {
        return Err(format!("invalid integer constant `{spelling}`"));
    }
    let digits = if digits.is_empty() { "0" } else { digits };
    let v = u128::from_str_radix(digits, radix)
        .map_err(|_| format!("integer constant `{spelling}` is too large"))?;
    let mut sf = int_suffix { unsigned: false, long: 0, decimal: radix == 10 };
    match suffix {
        "" => {}
        "u" => sf.unsigned = true,
        "l" => sf.long = 1,
        "ul" | "lu" => {
            sf.unsigned = true;
            sf.long = 1
        }
        "ll" => sf.long = 2,
        "ull" | "llu" => {
            sf.unsigned = true;
            sf.long = 2
        }
        "z" => sf.long = 1,
        "uz" | "zu" => {
            sf.unsigned = true;
            sf.long = 1
        }
        _ => return Err(format!("invalid suffix `{suffix}` on integer constant")),
    }
    Ok((v, sf))
}

/// Float suffix: 0 none (`double`), 1 `f`, 2 `l`.
pub fn float_literal(spelling: &str) -> Result<(f64, u8), String> {
    let s: String = spelling.chars().filter(|c| *c != '\'').collect();
    let lower = s.to_ascii_lowercase();
    let (body, kind) = if let Some(b) = lower.strip_suffix("f32").or(lower.strip_suffix('f')) {
        (b.to_string(), 1)
    } else if let Some(b) = lower.strip_suffix('l') {
        (b.to_string(), 2)
    } else if let Some(b) = lower.strip_suffix("f64") {
        (b.to_string(), 0)
    } else {
        (lower.clone(), 0)
    };
    if let Some(hex) = body.strip_prefix("0x") {
        let (mant, exp) =
            hex.split_once('p').ok_or("hexadecimal floating constant needs an exponent")?;
        let (int_part, frac_part) = mant.split_once('.').unwrap_or((mant, ""));
        let mut v = 0f64;
        for c in int_part.chars() {
            v = v * 16.0 + c.to_digit(16).ok_or("bad hex digit")? as f64;
        }
        let mut scale = 1.0 / 16.0;
        for c in frac_part.chars() {
            v += c.to_digit(16).ok_or("bad hex digit")? as f64 * scale;
            scale /= 16.0;
        }
        let e: i32 = exp.parse().map_err(|_| "bad exponent")?;
        return Ok((v * 2f64.powi(e), kind));
    }
    body.parse::<f64>()
        .map(|v| (v, kind))
        .map_err(|_| format!("invalid floating constant `{spelling}`"))
}

pub fn is_float_spelling(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("0x") {
        return lower.contains('.') || lower.contains('p');
    }
    lower.contains('.') || lower.contains('e') || (lower.ends_with('f') && !lower.starts_with("0x"))
}
