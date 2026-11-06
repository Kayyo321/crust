/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::util::{display_path, is_module_name, is_snake_ident};

// CODE_STYLE.md, section 2: enumerators are prefixed with their enum's name.
#[derive(Clone, Debug, PartialEq)]
enum TokenKind {
    TokenKindIdent(String),
    TokenKindString(String),
    TokenKindPunct(char),
    /** Numbers and character literals. Their contents never matter here. */
    TokenKindOther,
}

use TokenKind::*;

#[derive(Clone, Debug)]
struct token {
    kind: TokenKind,
    line: usize,
}

/** An `import ... from "<spec>"` or `export ... from "<spec>"` in a source file. */
#[derive(Clone, Debug, PartialEq)]
pub struct import_ref {
    pub spec: String,
    pub line: usize,
}

/** What a lexical scan of one `.crm` or `.cr` file found. */
#[derive(Clone, Debug, Default, PartialEq)]
pub struct source_scan {
    /** The `module <name>;` declaration and its line. */
    pub module_name: Option<(String, usize)>,
    pub imports: Vec<import_ref>,
    /** Paths named by `#include "..."`, as written. */
    pub includes: Vec<String>,
}

struct lexer<'a> {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    at_line_start: bool,
    tokens: Vec<token>,
    includes: &'a mut Vec<String>,
}

impl lexer<'_> {
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek(0)?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.at_line_start = true;
        }
        Some(c)
    }

    fn push(&mut self, kind: TokenKind, line: usize) {
        self.tokens.push(token { kind, line });
        self.at_line_start = false;
    }

    fn run(&mut self) {
        while let Some(c) = self.peek(0) {
            let line = self.line;
            match c {
                c if c.is_whitespace() => {
                    self.bump();
                }
                '/' if self.peek(1) == Some('/') => self.skip_line_comment(),
                '/' if self.peek(1) == Some('*') => self.skip_block_comment(),
                '#' if self.at_line_start => self.directive(),
                '"' => {
                    let text = self.quoted('"');
                    self.push(TokenKindString(text), line);
                }
                '\'' => {
                    self.quoted('\'');
                    self.push(TokenKindOther, line);
                }
                c if c.is_ascii_alphabetic() || c == '_' => {
                    let ident = self.take_while(|c| c.is_ascii_alphanumeric() || c == '_');
                    self.push(TokenKindIdent(ident), line);
                }
                c if c.is_ascii_digit()
                    || (c == '.' && self.peek(1).is_some_and(|d| d.is_ascii_digit())) =>
                {
                    self.number();
                    self.push(TokenKindOther, line);
                }
                c => {
                    self.bump();
                    self.push(TokenKindPunct(c), line);
                }
            }
        }
    }

    fn take_while(&mut self, keep: impl Fn(char) -> bool) -> String {
        let start = self.pos;
        while self.peek(0).is_some_and(&keep) {
            self.bump();
        }
        self.chars[start..self.pos].iter().collect()
    }

    fn skip_line_comment(&mut self) {
        while !matches!(self.peek(0), None | Some('\n')) {
            self.bump();
        }
    }

    fn skip_block_comment(&mut self) {
        self.pos += 2;
        while self.peek(0).is_some() {
            if self.peek(0) == Some('*') && self.peek(1) == Some('/') {
                self.pos += 2;
                return;
            }
            self.bump();
        }
    }

    /**
     * A pp-number, including C23 digit separators (`1'000`), so `'` does not start a
     * character literal.
     */
    fn number(&mut self) {
        while let Some(c) = self.peek(0) {
            let sign_after_exponent = (c == '+' || c == '-')
                && self.pos > 0
                && matches!(self.chars[self.pos - 1], 'e' | 'E' | 'p' | 'P');
            let separator = c == '\'' && self.peek(1).is_some_and(|d| d.is_ascii_alphanumeric());
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || sign_after_exponent || separator
            {
                self.bump();
            } else {
                return;
            }
        }
    }

    /**
     * Reads a string or character literal and returns its contents. Stops at the end of
     * the line if the literal is unterminated.
     */
    fn quoted(&mut self, quote: char) -> String {
        self.bump();
        let mut out = String::new();
        while let Some(c) = self.peek(0) {
            match c {
                '\n' => break,
                '\\' => {
                    self.bump();
                    if let Some(escaped) = self.bump() {
                        if escaped != '\n' {
                            out.push('\\');
                            out.push(escaped);
                        }
                    }
                }
                c if c == quote => {
                    self.bump();
                    break;
                }
                c => {
                    out.push(c);
                    self.bump();
                }
            }
        }
        out
    }

    /**
     * Skips a preprocessor directive, recording `#include "..."` paths. Macros and
     * conditional code are not interpreted: an `import` inside `#if 0` still counts.
     */
    fn directive(&mut self) {
        let mut text = String::new();
        while let Some(c) = self.peek(0) {
            if c == '\n' {
                break;
            }
            if c == '\\' && self.peek(1) == Some('\n') {
                self.bump();
                self.bump();
                continue;
            }
            if c == '/' && self.peek(1) == Some('*') {
                self.skip_block_comment();
                text.push(' ');
                continue;
            }
            if c == '/' && self.peek(1) == Some('/') {
                self.skip_line_comment();
                break;
            }
            text.push(c);
            self.bump();
        }
        let rest = text[1..].trim_start();
        if let Some(arg) = rest.strip_prefix("include") {
            let arg = arg.trim();
            if let Some(inner) = arg.strip_prefix('"').and_then(|a| a.split('"').next()) {
                self.includes.push(inner.to_string());
            }
        }
    }
}

/**
 * Scans a source file for its module declaration, imports, re-exports, and quoted
 * includes. Comments, string literals, and character literals are skipped.
 */
pub fn scan_source(text: &str) -> source_scan {
    let mut includes = Vec::new();
    let mut lex = lexer {
        chars: text.chars().collect(),
        pos: 0,
        line: 1,
        at_line_start: true,
        tokens: Vec::new(),
        includes: &mut includes,
    };
    lex.run();
    let tokens = std::mem::take(&mut lex.tokens);
    let mut scan = scan_tokens(&tokens);
    scan.includes = includes;
    scan
}

fn is_ident(t: Option<&token>, name: &str) -> bool {
    matches!(t, Some(token { kind: TokenKindIdent(s), .. }) if s == name)
}

fn is_punct(t: Option<&token>, c: char) -> bool {
    matches!(t, Some(token { kind: TokenKindPunct(p), .. }) if *p == c)
}

fn scan_tokens(tokens: &[token]) -> source_scan {
    let mut scan = source_scan::default();
    let mut depth = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        let at_statement_start = depth == 0
            && (i == 0 || [';', '}', ']'].iter().any(|c| is_punct(tokens.get(i - 1), *c)));
        let t = &tokens[i];
        if at_statement_start {
            if is_ident(Some(t), "module") && scan.module_name.is_none() {
                if let Some((name, next)) = module_decl(tokens, i + 1) {
                    scan.module_name = Some((name, t.line));
                    i = next;
                    continue;
                }
            }
            let reexport = is_ident(Some(t), "export")
                && (is_punct(tokens.get(i + 1), '{') || is_punct(tokens.get(i + 1), '*'));
            if is_ident(Some(t), "import") || reexport {
                i = from_clause(tokens, i + 1, &mut scan);
                continue;
            }
        }
        match t.kind {
            TokenKindPunct('{') => depth += 1,
            TokenKindPunct('}') => depth = depth.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }
    scan
}

/** Parses `a.b.c ;` starting at `i`. Returns the name and the index after `;`. */
fn module_decl(tokens: &[token], mut i: usize) -> Option<(String, usize)> {
    let mut name = String::new();
    loop {
        match tokens.get(i).map(|t| &t.kind) {
            Some(TokenKindIdent(seg)) => name.push_str(seg),
            _ => return None,
        }
        i += 1;
        if is_punct(tokens.get(i), ';') {
            return Some((name, i + 1));
        }
        if !is_punct(tokens.get(i), '.') {
            return None;
        }
        name.push('.');
        i += 1;
    }
}

/** Skips to the `;` that ends an import or re-export, recording `from "<spec>"`. */
fn from_clause(tokens: &[token], mut i: usize, scan: &mut source_scan) -> usize {
    while i < tokens.len() {
        if is_punct(tokens.get(i), ';') {
            return i + 1;
        }
        if is_ident(tokens.get(i), "from") {
            if let Some(token { kind: TokenKindString(spec), line }) = tokens.get(i + 1) {
                scan.imports.push(import_ref { spec: spec.clone(), line: *line });
            }
        }
        i += 1;
    }
    i
}

/** Where an import specifier points. */
// CODE_STYLE.md, section 2: enumerators are prefixed with their enum's name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImportTarget {
    /** A module in the same package. */
    ImportTargetLocal(String),
    /** A module in a dependency package. */
    ImportTargetExtern { package: String, module: String },
}

pub use ImportTarget::*;

impl ImportTarget {
    /** The key recorded in incremental state: `name` or `package/name`. */
    pub fn key(&self) -> String {
        match self {
            ImportTargetLocal(m) => m.clone(),
            ImportTargetExtern { package, module } => format!("{package}/{module}"),
        }
    }
}

/**
 * Resolves a specifier (D-029). `"name"` is a same-package module, `"pkg/name"` a module
 * of a dependency, and `"pkg"` the root module of a dependency.
 */
pub fn resolve_spec(
    spec: &str,
    local: &BTreeSet<String>,
    deps: &BTreeSet<String>,
) -> Result<ImportTarget, String> {
    if let Some((package, module)) = spec.split_once('/') {
        if !is_snake_ident(package) || !is_module_name(module) {
            return Err(format!(
                "invalid import specifier \"{spec}\": expected \"package/module.name\""
            ));
        }
        if !deps.contains(package) {
            return Err(format!(
                "import from \"{spec}\": package `{package}` is not listed under [dependencies] \
                 in crepe.toml"
            ));
        }
        return Ok(ImportTargetExtern { package: package.to_string(), module: module.to_string() });
    }
    if !is_module_name(spec) {
        return Err(format!(
            "invalid import specifier \"{spec}\": module names are dotted snake_case"
        ));
    }
    match (local.contains(spec), deps.contains(spec)) {
        (true, true) => Err(format!(
            "import from \"{spec}\" is ambiguous: `{spec}` is both a module of this package and a \
             dependency\nhelp: rename the module, or write \"{spec}/{spec}\" for the \
             dependency's root"
        )),
        (true, false) => Ok(ImportTargetLocal(spec.to_string())),
        (false, true) => {
            Ok(ImportTargetExtern { package: spec.to_string(), module: spec.to_string() })
        }
        (false, false) => Err(format!(
            "import from \"{spec}\": no module named `{spec}` in this package, and no dependency \
             named `{spec}` in crepe.toml"
        )),
    }
}

/** One scanned source file: a `.crm` module or a `.cr` file. */
#[derive(Clone, Debug)]
pub struct source_file {
    /** Absolute path. */
    pub path: PathBuf,
    /** Path relative to the package directory, for messages and crust arguments. */
    pub rel: String,
    pub scan: source_scan,
}

#[derive(Clone, Debug)]
pub struct module_node {
    pub name: String,
    pub file: source_file,
    pub imports: Vec<ImportTarget>,
    /** Indices of same-package modules this module imports. */
    pub local_deps: Vec<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct module_graph {
    pub modules: Vec<module_node>,
    pub by_name: BTreeMap<String, usize>,
    pub by_path: BTreeMap<PathBuf, usize>,
}

/** Lists every `.crm` file under `dir`, sorted, skipping hidden directories. */
pub fn find_modules(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            if path.is_dir() && !hidden {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "crm") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

pub fn read_source(package_dir: &Path, path: &Path) -> Result<source_file, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("could not read `{}`: {e}", display_path(package_dir, path)))?;
    Ok(source_file {
        path: path.to_path_buf(),
        rel: display_path(package_dir, path),
        scan: scan_source(&text),
    })
}

/** The module name a file's path implies, if it follows the convention. */
fn expected_name(package: &str, package_dir: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(package_dir.join("src")).ok()?;
    let rel = rel.with_extension("");
    let parts: Vec<String> =
        rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if parts == ["main"] || parts == ["lib"] {
        return Some(package.to_string());
    }
    Some(parts.join("."))
}

/** Builds a package's module graph. Returns the graph and warnings, or every error. */
pub fn build_graph(
    package: &str,
    package_dir: &Path,
    files: Vec<source_file>,
    deps: &BTreeSet<String>,
) -> Result<(module_graph, Vec<String>), Vec<String>> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut graph = module_graph::default();
    for file in files {
        let Some((name, line)) = file.scan.module_name.clone() else {
            errors.push(format!("{}: missing `module <name>;` declaration", file.rel));
            continue;
        };
        if !is_module_name(&name) {
            errors.push(format!(
                "{}:{line}: invalid module name `{name}`: segments must be snake_case",
                file.rel
            ));
            continue;
        }
        if let Some(&other) = graph.by_name.get(&name) {
            let first = &graph.modules[other].file.rel;
            errors.push(format!(
                "duplicate module name `{name}`: declared in `{first}` and `{}`",
                file.rel
            ));
            continue;
        }
        if let Some(expected) = expected_name(package, package_dir, &file.path) {
            if expected != name {
                warnings.push(format!(
                    "{}: module `{name}` should be named `{expected}` to match its path",
                    file.rel
                ));
            }
        }
        let index = graph.modules.len();
        graph.by_name.insert(name.clone(), index);
        graph.by_path.insert(file.path.clone(), index);
        graph.modules.push(module_node { name, file, imports: Vec::new(), local_deps: Vec::new() });
    }
    let local: BTreeSet<String> = graph.by_name.keys().cloned().collect();
    for node in &mut graph.modules {
        let (imports, errs) = resolve_imports(&node.file, &local, deps);
        errors.extend(errs);
        node.local_deps = imports
            .iter()
            .filter_map(|t| match t {
                ImportTargetLocal(m) => graph.by_name.get(m).copied(),
                ImportTargetExtern { .. } => None,
            })
            .collect();
        node.imports = imports;
    }
    if errors.is_empty() {
        if let Some(cycle) = find_cycle(&graph) {
            errors.push(format!(
                "import cycle: {}\nhelp: move the shared declarations into a third module",
                cycle.join(" -> ")
            ));
        }
    }
    if errors.is_empty() { Ok((graph, warnings)) } else { Err(errors) }
}

/** Resolves every import of a file, deduplicated, in first-seen order. */
pub fn resolve_imports(
    file: &source_file,
    local: &BTreeSet<String>,
    deps: &BTreeSet<String>,
) -> (Vec<ImportTarget>, Vec<String>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for import in &file.scan.imports {
        match resolve_spec(&import.spec, local, deps) {
            Ok(target) if !out.contains(&target) => out.push(target),
            Ok(_) => {}
            Err(e) => errors.push(format!("{}:{}: {e}", file.rel, import.line)),
        }
    }
    let own = file.scan.module_name.as_ref().map(|(n, _)| ImportTargetLocal(n.clone()));
    if let Some(own) = own {
        if out.contains(&own) {
            errors.push(format!("{}: module `{}` imports itself", file.rel, own.key()));
        }
    }
    (out, errors)
}

/** Finds an import cycle, returned as module names with the first repeated at the end. */
pub fn find_cycle(graph: &module_graph) -> Option<Vec<String>> {
    // 0 = unvisited, 1 = on the current path, 2 = done.
    let mut color = vec![0u8; graph.modules.len()];
    let mut path = Vec::new();
    for start in 0..graph.modules.len() {
        if color[start] == 0 {
            if let Some(cycle) = cycle_from(graph, start, &mut color, &mut path) {
                return Some(cycle.iter().map(|&i| graph.modules[i].name.clone()).collect());
            }
        }
    }
    None
}

fn cycle_from(
    graph: &module_graph,
    node: usize,
    color: &mut [u8],
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    color[node] = 1;
    path.push(node);
    for &next in &graph.modules[node].local_deps {
        if color[next] == 1 {
            let start = path.iter().position(|&p| p == next).unwrap_or(0);
            let mut cycle = path[start..].to_vec();
            cycle.push(next);
            return Some(cycle);
        }
        if color[next] == 0 {
            if let Some(cycle) = cycle_from(graph, next, color, path) {
                return Some(cycle);
            }
        }
    }
    path.pop();
    color[node] = 2;
    None
}

/** Every module reachable from `roots`, each after the modules it imports. */
pub fn dependency_order(graph: &module_graph, roots: &[usize]) -> Vec<usize> {
    let mut seen = vec![false; graph.modules.len()];
    let mut order = Vec::new();
    for &root in roots {
        visit_post(graph, root, &mut seen, &mut order);
    }
    order
}

fn visit_post(graph: &module_graph, node: usize, seen: &mut [bool], order: &mut Vec<usize>) {
    if seen[node] {
        return;
    }
    seen[node] = true;
    for &next in &graph.modules[node].local_deps {
        visit_post(graph, next, seen, order);
    }
    order.push(node);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specs(scan: &source_scan) -> Vec<&str> {
        scan.imports.iter().map(|i| i.spec.as_str()).collect()
    }

    #[test]
    fn reads_module_imports_and_reexports() {
        let text = "/**\n* header\n*/\nmodule net.http;\n\n\
                    #include <stdio.h>\n#include \"util.hr\"\n\
                    import { a, b as c } from \"net.socket\";\nimport * as m from \"mathx/core\";\n\
                    export { x } from \"net.url\";\nexport * from \"mathx\";\n\
                    export int f(void) { return 0; }\n";
        let scan = scan_source(text);
        assert_eq!(scan.module_name, Some(("net.http".to_string(), 4)));
        assert_eq!(specs(&scan), vec!["net.socket", "mathx/core", "net.url", "mathx"]);
        assert_eq!(scan.includes, vec!["util.hr".to_string()]);
        assert_eq!(scan.imports[0].line, 8);
    }

    #[test]
    fn skips_comments_and_strings() {
        let text = "module app;\n// import { a } from \"line_comment\";\n\
                    /* import { b } from \"block_comment\"; */\n\
                    const char *s = \"import { c } from \\\"string\\\";\";\n\
                    char q = '\"'; int n = 1'000'000;\nimport { d } from \"real\";\n";
        let scan = scan_source(text);
        assert_eq!(scan.module_name, Some(("app".to_string(), 1)));
        assert_eq!(specs(&scan), vec!["real"]);
    }

    #[test]
    fn ignores_imports_inside_functions_and_declarations() {
        let text = "module app;\nint f(void) { import { x } from \"inner\"; }\n\
                    export struct p { int from; };\nimport { y } from \"outer\";\n\
                    #define BAD import { z } from \"macro\";\n";
        let scan = scan_source(text);
        assert_eq!(specs(&scan), vec!["outer"]);
    }

    #[test]
    fn reads_attributed_reexports() {
        let text =
            "module app;\n[[crust::c_abi]] export int g(void);\nexport { g } from \"lib.g\";\n";
        assert_eq!(specs(&scan_source(text)), vec!["lib.g"]);
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn resolves_specifiers() {
        let local = set(&["geometry", "net.http"]);
        let deps = set(&["mathx"]);
        assert_eq!(
            resolve_spec("geometry", &local, &deps),
            Ok(ImportTargetLocal("geometry".into()))
        );
        let root = ImportTargetExtern { package: "mathx".into(), module: "mathx".into() };
        assert_eq!(resolve_spec("mathx", &local, &deps), Ok(root));
        let core = ImportTargetExtern { package: "mathx".into(), module: "core".into() };
        assert_eq!(resolve_spec("mathx/core", &local, &deps), Ok(core));
        assert!(
            resolve_spec("other/x", &local, &deps)
                .unwrap_err()
                .contains("not listed under [dependencies]")
        );
        assert!(resolve_spec("missing", &local, &deps).unwrap_err().contains("no module named"));
        assert!(resolve_spec("Bad", &local, &deps).is_err());
        let both = set(&["mathx"]);
        assert!(resolve_spec("mathx", &both, &deps).unwrap_err().contains("ambiguous"));
    }

    fn file(rel: &str, text: &str) -> source_file {
        source_file {
            path: Path::new("/pkg").join(rel),
            rel: rel.to_string(),
            scan: scan_source(text),
        }
    }

    #[test]
    fn detects_cycles() {
        let files = vec![
            file("src/a.crm", "module a;\nimport { x } from \"b\";\n"),
            file("src/b.crm", "module b;\nimport { y } from \"c\";\n"),
            file("src/c.crm", "module c;\nimport { z } from \"a\";\n"),
        ];
        let errors = build_graph("pkg", Path::new("/pkg"), files, &BTreeSet::new()).unwrap_err();
        assert!(errors[0].contains("import cycle: a -> b -> c -> a"), "{errors:?}");
    }

    #[test]
    fn reports_duplicates_unknown_imports_and_undeclared_packages() {
        let files = vec![
            file(
                "src/a.crm",
                "module a;\nimport { x } from \"nope\";\nimport { y } from \"ext/m\";\n",
            ),
            file("src/b.crm", "module a;\n"),
            file("src/c.crm", "int main(void) { return 0; }\n"),
        ];
        let errors = build_graph("pkg", Path::new("/pkg"), files, &BTreeSet::new()).unwrap_err();
        let all = errors.join("\n");
        assert!(all.contains("duplicate module name `a`"), "{all}");
        assert!(all.contains("src/a.crm:2: import from \"nope\""), "{all}");
        assert!(all.contains("package `ext` is not listed"), "{all}");
        assert!(all.contains("src/c.crm: missing `module"), "{all}");
    }

    #[test]
    fn orders_modules_after_their_imports_and_warns_on_path_mismatch() {
        let files = vec![
            file(
                "src/main.crm",
                "module pkg;\nimport { g } from \"geo.shapes\";\nimport { u } from \"util\";\n",
            ),
            file("src/geo/shapes.crm", "module geo.shapes;\nimport { u } from \"util\";\n"),
            file("src/utilx.crm", "module util;\n"),
            file("src/lone.crm", "module lone;\n"),
        ];
        let (graph, warnings) =
            build_graph("pkg", Path::new("/pkg"), files, &BTreeSet::new()).unwrap();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("should be named `utilx`"));
        let root = graph.by_name["pkg"];
        let order: Vec<_> = dependency_order(&graph, &[root])
            .iter()
            .map(|&i| graph.modules[i].name.clone())
            .collect();
        assert_eq!(order, vec!["util", "geo.shapes", "pkg"]);
    }
}
