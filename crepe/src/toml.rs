/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::collections::BTreeSet;

/** A TOML value. Floats and date-times are not supported: crepe's files do not use them. */
// CODE_STYLE.md, section 2: enumerators are prefixed with their enum's name.
#[derive(Clone, Debug, PartialEq)]
pub enum TomlValue {
    TomlValueString(String),
    TomlValueInteger(i64),
    TomlValueBoolean(bool),
    TomlValueArray(Vec<TomlValue>),
    TomlValueTable(toml_table),
}

use TomlValue::*;

/** A table that keeps its keys in source order. */
#[derive(Clone, Debug, Default, PartialEq)]
pub struct toml_table {
    pub entries: Vec<(String, TomlValue)>,
}

impl toml_table {
    pub fn get(&self, key: &str) -> Option<&TomlValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn get_mut(&mut self, key: &str) -> Option<&mut TomlValue> {
        self.entries.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }
}

impl TomlValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            TomlValueString(_) => "string",
            TomlValueInteger(_) => "integer",
            TomlValueBoolean(_) => "boolean",
            TomlValueArray(_) => "array",
            TomlValueTable(_) => "table",
        }
    }
}

/** Quotes `text` as a TOML basic string. */
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/** Parses a TOML document. Errors read `line N: message`. */
pub fn parse(text: &str) -> Result<toml_table, String> {
    let mut p = parser { chars: text.chars().collect(), pos: 0, line: 1 };
    let mut root = toml_table::default();
    let mut current: Vec<String> = Vec::new();
    let mut defined: BTreeSet<Vec<String>> = BTreeSet::new();
    p.run(&mut root, &mut current, &mut defined)
        .map_err(|msg| format!("line {}: {msg}", p.line))?;
    Ok(root)
}

struct parser {
    chars: Vec<char>,
    pos: usize,
    line: usize,
}

impl parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn starts_with(&self, text: &str) -> bool {
        text.chars().enumerate().all(|(i, c)| self.peek_at(i) == Some(c))
    }

    fn expect(&mut self, want: char) -> Result<(), String> {
        match self.peek() {
            Some(c) if c == want => {
                self.bump();
                Ok(())
            }
            Some(c) => Err(format!("expected `{want}`, found `{c}`")),
            None => Err(format!("expected `{want}`, found end of file")),
        }
    }

    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.bump();
        }
    }

    fn skip_comment(&mut self) {
        if self.peek() == Some('#') {
            while !matches!(self.peek(), None | Some('\n')) {
                self.bump();
            }
        }
    }

    /** Skips blank lines, comments, and whitespace, including newlines. */
    fn skip_trivia(&mut self) {
        loop {
            self.skip_spaces();
            self.skip_comment();
            match self.peek() {
                Some('\n') => {
                    self.bump();
                }
                Some('\r') if self.peek_at(1) == Some('\n') => {
                    self.bump();
                }
                _ => return,
            }
        }
    }

    fn expect_line_end(&mut self) -> Result<(), String> {
        self.skip_spaces();
        self.skip_comment();
        if self.peek() == Some('\r') {
            self.bump();
        }
        match self.peek() {
            None => Ok(()),
            Some('\n') => {
                self.bump();
                Ok(())
            }
            Some(c) => Err(format!("expected the end of the line, found `{c}`")),
        }
    }

    fn run(
        &mut self,
        root: &mut toml_table,
        current: &mut Vec<String>,
        defined: &mut BTreeSet<Vec<String>>,
    ) -> Result<(), String> {
        loop {
            self.skip_trivia();
            match self.peek() {
                None => return Ok(()),
                Some('[') => self.table_header(root, current, defined)?,
                Some(_) => {
                    let keys = self.key_path()?;
                    self.skip_spaces();
                    self.expect('=')?;
                    self.skip_spaces();
                    let value = self.value()?;
                    let table = navigate(root, current)?;
                    insert_dotted(table, &keys, value)?;
                }
            }
            self.expect_line_end()?;
        }
    }

    fn table_header(
        &mut self,
        root: &mut toml_table,
        current: &mut Vec<String>,
        defined: &mut BTreeSet<Vec<String>>,
    ) -> Result<(), String> {
        self.expect('[')?;
        let is_array = self.peek() == Some('[');
        if is_array {
            self.bump();
        }
        self.skip_spaces();
        let path = self.key_path()?;
        self.skip_spaces();
        self.expect(']')?;
        if is_array {
            self.expect(']')?;
            let (last, parents) = path.split_last().ok_or("empty table name")?;
            let parent = navigate(root, parents)?;
            match parent.get_mut(last) {
                None => {
                    let tables = vec![TomlValueTable(toml_table::default())];
                    parent.entries.push((last.clone(), TomlValueArray(tables)));
                }
                Some(TomlValueArray(items))
                    if items.iter().all(|v| matches!(v, TomlValueTable(_))) =>
                {
                    items.push(TomlValueTable(toml_table::default()));
                }
                Some(_) => return Err(format!("`{}` is not an array of tables", path.join("."))),
            }
            // Sub-tables of the previous element may be defined again under the new one.
            defined.retain(|d| !d.starts_with(&path) || d.len() == path.len());
        } else {
            if !defined.insert(path.clone()) {
                return Err(format!("table `{}` is defined twice", path.join(".")));
            }
            navigate(root, &path)?;
        }
        *current = path;
        Ok(())
    }

    fn key_path(&mut self) -> Result<Vec<String>, String> {
        let mut keys = vec![self.key()?];
        loop {
            self.skip_spaces();
            if self.peek() != Some('.') {
                return Ok(keys);
            }
            self.bump();
            self.skip_spaces();
            keys.push(self.key()?);
        }
    }

    fn key(&mut self) -> Result<String, String> {
        match self.peek() {
            Some('"') => self.basic_string(),
            Some('\'') => self.literal_string(),
            _ => {
                let start = self.pos;
                let is_bare = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
                while self.peek().is_some_and(is_bare) {
                    self.bump();
                }
                if start == self.pos {
                    return Err(match self.peek() {
                        Some(c) => format!("expected a key, found `{c}`"),
                        None => "expected a key, found end of file".to_string(),
                    });
                }
                Ok(self.chars[start..self.pos].iter().collect())
            }
        }
    }

    fn value(&mut self) -> Result<TomlValue, String> {
        match self.peek() {
            Some('"') if self.starts_with("\"\"\"") => self.multiline_basic().map(TomlValueString),
            Some('\'') if self.starts_with("'''") => self.multiline_literal().map(TomlValueString),
            Some('"') => self.basic_string().map(TomlValueString),
            Some('\'') => self.literal_string().map(TomlValueString),
            Some('[') => self.array(),
            Some('{') => self.inline_table(),
            Some('t') if self.starts_with("true") => {
                self.pos += 4;
                Ok(TomlValueBoolean(true))
            }
            Some('f') if self.starts_with("false") => {
                self.pos += 5;
                Ok(TomlValueBoolean(false))
            }
            Some(c) if c.is_ascii_digit() || c == '+' || c == '-' => self.integer(),
            Some(c) => Err(format!("expected a value, found `{c}`")),
            None => Err("expected a value, found end of file".to_string()),
        }
    }

    fn integer(&mut self) -> Result<TomlValue, String> {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || "+-_.:".contains(c)) {
            self.bump();
        }
        let raw: String = self.chars[start..self.pos].iter().collect();
        let digits = raw.replace('_', "");
        let unsigned = digits.trim_start_matches(['+', '-']);
        let valid = !unsigned.is_empty()
            && unsigned.chars().all(|c| c.is_ascii_digit())
            && !raw.starts_with('_')
            && !raw.ends_with('_')
            && !raw.contains("__");
        if !valid {
            return Err(format!(
                "unsupported value `{raw}` (only integers, strings, booleans, \
                                arrays, and tables are supported)"
            ));
        }
        digits
            .parse::<i64>()
            .map(TomlValueInteger)
            .map_err(|_| format!("integer `{raw}` is out of range"))
    }

    fn escape(&mut self) -> Result<char, String> {
        let c = self.bump().ok_or("unterminated string")?;
        let simple = match c {
            'b' => Some('\u{8}'),
            't' => Some('\t'),
            'n' => Some('\n'),
            'f' => Some('\u{c}'),
            'r' => Some('\r'),
            '"' => Some('"'),
            '\\' => Some('\\'),
            _ => None,
        };
        if let Some(c) = simple {
            return Ok(c);
        }
        let width = match c {
            'u' => 4,
            'U' => 8,
            other => return Err(format!("unknown escape `\\{other}`")),
        };
        let mut code = 0u32;
        for _ in 0..width {
            let digit = self.bump().and_then(|d| d.to_digit(16)).ok_or("bad unicode escape")?;
            code = code * 16 + digit;
        }
        char::from_u32(code).ok_or_else(|| format!("invalid unicode scalar U+{code:X}"))
    }

    fn basic_string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            if matches!(self.peek(), None | Some('\n')) {
                return Err("unterminated string".to_string());
            }
            match self.bump() {
                None => return Err("unterminated string".to_string()),
                Some('"') => return Ok(out),
                Some('\\') => out.push(self.escape()?),
                Some(c) if (c as u32) < 0x20 && c != '\t' => {
                    return Err("control character in string".to_string());
                }
                Some(c) => out.push(c),
            }
        }
    }

    fn literal_string(&mut self) -> Result<String, String> {
        self.expect('\'')?;
        let mut out = String::new();
        loop {
            if matches!(self.peek(), None | Some('\n')) {
                return Err("unterminated string".to_string());
            }
            match self.bump() {
                None => return Err("unterminated string".to_string()),
                Some('\'') => return Ok(out),
                Some(c) => out.push(c),
            }
        }
    }

    fn skip_first_newline(&mut self) {
        if self.peek() == Some('\n') {
            self.bump();
        } else if self.starts_with("\r\n") {
            self.bump();
            self.bump();
        }
    }

    fn multiline_basic(&mut self) -> Result<String, String> {
        self.pos += 3;
        self.skip_first_newline();
        let mut out = String::new();
        loop {
            if self.starts_with("\"\"\"") {
                self.pos += 3;
                return Ok(out);
            }
            match self.bump() {
                None => return Err("unterminated string".to_string()),
                Some('\\') if matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) => {
                    // A line-ending backslash trims the newline and leading whitespace.
                    while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
                        self.bump();
                    }
                }
                Some('\\') => out.push(self.escape()?),
                Some(c) => out.push(c),
            }
        }
    }

    fn multiline_literal(&mut self) -> Result<String, String> {
        self.pos += 3;
        self.skip_first_newline();
        let mut out = String::new();
        loop {
            if self.starts_with("'''") {
                self.pos += 3;
                return Ok(out);
            }
            out.push(self.bump().ok_or("unterminated string")?);
        }
    }

    fn array(&mut self) -> Result<TomlValue, String> {
        self.expect('[')?;
        let mut items = Vec::new();
        loop {
            self.skip_trivia();
            if self.peek() == Some(']') {
                self.bump();
                return Ok(TomlValueArray(items));
            }
            items.push(self.value()?);
            self.skip_trivia();
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(']') => {}
                _ => return Err("expected `,` or `]` in array".to_string()),
            }
        }
    }

    fn inline_table(&mut self) -> Result<TomlValue, String> {
        self.expect('{')?;
        let mut table = toml_table::default();
        self.skip_spaces();
        if self.peek() == Some('}') {
            self.bump();
            return Ok(TomlValueTable(table));
        }
        loop {
            self.skip_spaces();
            let keys = self.key_path()?;
            self.skip_spaces();
            self.expect('=')?;
            self.skip_spaces();
            let value = self.value()?;
            insert_dotted(&mut table, &keys, value)?;
            self.skip_spaces();
            match self.bump() {
                Some(',') => {}
                Some('}') => return Ok(TomlValueTable(table)),
                _ => return Err("expected `,` or `}` in inline table".to_string()),
            }
        }
    }
}

/**
 * Walks to the table named by `path`, creating missing tables. An array of tables
 * resolves to its last element, as TOML requires.
 */
fn navigate<'t>(root: &'t mut toml_table, path: &[String]) -> Result<&'t mut toml_table, String> {
    let mut table = root;
    for (depth, key) in path.iter().enumerate() {
        if table.get(key).is_none() {
            table.entries.push((key.clone(), TomlValueTable(toml_table::default())));
        }
        let next = table.get_mut(key).ok_or("internal: missing key")?;
        table = match next {
            TomlValueTable(t) => t,
            TomlValueArray(items) => match items.last_mut() {
                Some(TomlValueTable(t)) => t,
                _ => return Err(format!("`{}` is not a table", path[..=depth].join("."))),
            },
            _ => return Err(format!("`{}` is not a table", path[..=depth].join("."))),
        };
    }
    Ok(table)
}

fn insert_dotted(table: &mut toml_table, keys: &[String], value: TomlValue) -> Result<(), String> {
    let (last, parents) = keys.split_last().ok_or("empty key")?;
    let target = navigate(table, parents)?;
    if target.get(last).is_some() {
        return Err(format!("key `{}` is defined twice", keys.join(".")));
    }
    target.entries.push((last.clone(), value));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ensure::ensure;

    fn s(text: &str) -> TomlValue {
        TomlValueString(text.to_string())
    }

    /// The table at `value`, or a failure that says what was there instead.
    fn table_of<'a>(value: Option<&'a TomlValue>, what: &str) -> &'a toml_table {
        let found = ensure(value)
            .named(what)
            .is_some()
            .satisfies("is a table", |v| matches!(v, TomlValueTable(_)))
            .into_value();
        match found {
            TomlValueTable(table) => table,
            _ => unreachable!("`satisfies` reported a value that is not a table"),
        }
    }

    /// The array at `value`, or a failure that says what was there instead.
    fn array_of<'a>(value: Option<&'a TomlValue>, what: &str) -> &'a [TomlValue] {
        let found = ensure(value)
            .named(what)
            .is_some()
            .satisfies("is an array", |v| matches!(v, TomlValueArray(_)))
            .into_value();
        match found {
            TomlValueArray(items) => items,
            _ => unreachable!("`satisfies` reported a value that is not an array"),
        }
    }

    fn parsed(text: &str) -> toml_table {
        ensure(parse(text)).named("parsing the document").is_ok().into_value()
    }

    #[test]
    fn parses_tables_strings_and_scalars() {
        let doc = parsed(
            "# comment\n[package]\nname = \"app\" # trailing\nversion = '0.1.0'\n\
             count = 1_000\nneg = -3\nflag = true\n",
        );
        let pkg = table_of(doc.get("package"), "[package]");
        ensure(pkg.get("name")).is(Some(&s("app")));
        ensure(pkg.get("version")).is(Some(&s("0.1.0")));
        ensure(pkg.get("count")).is(Some(&TomlValueInteger(1000)));
        ensure(pkg.get("neg")).is(Some(&TomlValueInteger(-3)));
        ensure(pkg.get("flag")).is(Some(&TomlValueBoolean(true)));
    }

    #[test]
    fn parses_escapes() {
        let doc = parsed(r#"a = "x\"y\\z\né""#);
        ensure(doc.get("a")).is(Some(&s("x\"y\\z\né")));
    }

    #[test]
    fn parses_multiline_arrays_and_inline_tables() {
        let doc = parsed(
            "srcs = [\n  \"a.cr\", # first\n  \"b.cr\",\n]\n\
             dep = { path = \"../x\", nested.k = 1 }\n",
        );
        ensure(doc.get("srcs")).is(Some(&TomlValueArray(vec![s("a.cr"), s("b.cr")])));
        let dep = table_of(doc.get("dep"), "dep");
        ensure(dep.get("path")).is(Some(&s("../x")));
        let nested = table_of(dep.get("nested"), "dep.nested");
        ensure(nested.get("k")).is(Some(&TomlValueInteger(1)));
    }

    #[test]
    fn parses_arrays_of_tables() {
        let doc = parsed("[[bin]]\nname = \"a\"\n[[bin]]\nname = \"b\"\n[bin.extra]\nx = 1\n");
        let bins = array_of(doc.get("bin"), "[[bin]]");
        ensure(bins).named("[[bin]] tables").has_len(2);
        let second = table_of(bins.get(1), "second [[bin]]");
        ensure(second.get("name")).is(Some(&s("b")));
        ensure(second.get("extra")).named("[bin.extra]").is_some();
    }

    #[test]
    fn parses_dotted_keys_and_headers() {
        let doc = parsed("a.b = 1\n[dependencies.util]\npath = \"../util\"\n");
        let a = table_of(doc.get("a"), "a");
        ensure(a.get("b")).is(Some(&TomlValueInteger(1)));
        let deps = table_of(doc.get("dependencies"), "[dependencies]");
        ensure(deps.get("util")).named("[dependencies.util]").is_some();
    }

    #[test]
    fn parses_multiline_strings() {
        let doc = parsed("a = \"\"\"\nline1\nline2\"\"\"\nb = '''\nraw\\n'''\n");
        ensure(doc.get("a")).is(Some(&s("line1\nline2")));
        ensure(doc.get("b")).is(Some(&s("raw\\n")));
    }

    #[test]
    fn rejects_duplicate_keys_and_tables() {
        ensure(parse("a = 1\na = 2\n")).is_err().contains("defined twice");
        ensure(parse("[x]\n[x]\n")).is_err().contains("defined twice");
    }

    #[test]
    fn rejects_malformed_input_with_line_numbers() {
        ensure(parse("a = 1\nb = \"open\n")).is_err().starts_with("line 2:");
        for bad in ["a = 1 2\n", "a = 1.5\n", "a = [1, 2\n", "= 1\n"] {
            ensure(parse(bad)).named(format!("parse of {bad:?}")).is_err();
        }
    }

    #[test]
    fn quote_round_trips() {
        let text = "a \"b\" \\ c\n\u{1}";
        let doc = parsed(&format!("k = {}\n", quote(text)));
        ensure(doc.get("k")).is(Some(&s(text)));
    }
}
