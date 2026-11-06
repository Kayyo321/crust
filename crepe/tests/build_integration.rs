/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/

#[cfg(unix)]
mod build_tests {
    use ensure::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    /**
     * A stand-in for `crust` that follows the contract crepe builds against: `-c` writes
     * the object and `<module>.crmi` next to `-o`, rewriting the interface only when its
     * content changes. The interface is the file's `export` lines, so a body-only edit
     * keeps the hash. Imports must resolve from the `-o` directory or `--extern`, which
     * fails the compile when crepe gets the dependency order wrong.
     */
    const FakeCrust: &str = r##"#!/bin/sh
# Ko, (Baczynski-Bruce) 2026; Apache 2.0
echo "$*" >> "$FAKE_CRUST_LOG"
emit=exe; out=""; externs=""; inputs=""
while [ $# -gt 0 ]; do
    case "$1" in
        -c) emit=obj ;;
        --emit=*) emit="${1#--emit=}" ;;
        -o) shift; out="$1" ;;
        --extern) shift; externs="$externs $1" ;;
        -I|-L|-l) shift ;;
        -O*|-g) ;;
        *) inputs="$inputs $1" ;;
    esac
    shift
done
check_imports() {
    outdir=$(dirname "$out")
    for spec in $(sed -n 's/^import .* from "\(.*\)";.*$/\1/p' "$1"); do
        case "$spec" in
            */*) pkg=${spec%%/*}; mod=${spec#*/} ;;
            *) pkg=""; mod=$spec ;;
        esac
        if [ -z "$pkg" ] && [ -f "$outdir/$mod.crmi" ]; then continue; fi
        [ -z "$pkg" ] && pkg=$spec
        found=""
        for e in $externs; do
            if [ "${e%%=*}" = "$pkg" ] && [ -f "${e#*=}/$mod.crmi" ]; then found=1; fi
        done
        if [ -z "$found" ]; then
            echo "$1: error: cannot find the interface of \"$spec\"" >&2
            exit 1
        fi
    done
    if grep -q COMPILE_ERROR "$1"; then
        echo "$1:1: error: COMPILE_ERROR" >&2
        exit 1
    fi
}
write_crmi() {
    name=$(sed -n 's/^module \([a-z0-9_.]*\);.*/\1/p' "$1" | head -n 1)
    crmi="$(dirname "$out")/$name.crmi"
    h=$(grep '^export' "$1" | cksum | cut -d' ' -f1)
    tmp="$crmi.tmp.$$"
    {
        printf 'CRMI\001\000\000\000'
        i=0
        while [ $i -lt 4 ]; do
            printf "\\$(printf %03o $(( (h >> (8 * i)) & 255 )))"
            i=$((i + 1))
        done
        printf '\000\000\000\000'
        grep '^export' "$1"
    } > "$tmp"
    if [ -f "$crmi" ] && cmp -s "$tmp" "$crmi"; then rm -f "$tmp"; else mv "$tmp" "$crmi"; fi
}
src=$(echo $inputs)
case "$emit" in
    obj)
        check_imports "$src"
        cp "$src" "$out"
        case "$src" in *.crm) write_crmi "$src" ;; esac ;;
    llvm-ir) check_imports "$src"; : > "$out" ;;
    header) check_imports "$src"; printf '/* generated */\n' > "$out" ;;
    staticlib) cat $inputs > "$out" ;;
    exe)
        if cat $inputs | grep -q TEST_FAIL; then code=1; else code=0; fi
        printf '#!/bin/sh\necho "ran $0 $*"\nexit %s\n' "$code" > "$out"
        chmod +x "$out" ;;
esac
exit 0
"##;

    struct sandbox {
        dir: PathBuf,
        crust: PathBuf,
        log: PathBuf,
    }

    impl sandbox {
        fn new(name: &str) -> sandbox {
            let dir =
                std::env::temp_dir().join(format!("crepe-test-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            let crust = dir.join("fake_crust");
            fs::write(&crust, FakeCrust).unwrap();
            fs::set_permissions(&crust, fs::Permissions::from_mode(0o755)).unwrap();
            let log = dir.join("crust.log");
            sandbox { dir, crust, log }
        }

        fn write(&self, rel: &str, text: &str) {
            let path = self.dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        fn path(&self, rel: &str) -> PathBuf {
            self.dir.join(rel)
        }

        fn crepe(&self, cwd: &str, args: &[&str]) -> Output {
            let _ = fs::remove_file(&self.log);
            Command::new(env!("CARGO_BIN_EXE_crepe"))
                .args(args)
                .current_dir(self.dir.join(cwd))
                .env("CRUST", &self.crust)
                .env("FAKE_CRUST_LOG", &self.log)
                .env("CRUST_HOME", self.dir.join("home"))
                .output()
                .unwrap()
        }

        fn crepe_ok(&self, cwd: &str, args: &[&str]) -> Output {
            let out = self.crepe(cwd, args);
            ensure(out).named(format!("crepe {args:?}")).succeeds().into_value()
        }

        fn calls(&self) -> Vec<String> {
            fs::read_to_string(&self.log).unwrap_or_default().lines().map(String::from).collect()
        }

        /** Source files compiled with `-c`, in order. */
        fn compiled(&self) -> Vec<String> {
            self.calls()
                .iter()
                .filter_map(|c| {
                    c.strip_prefix("-c ").map(|r| r.split(' ').next().unwrap().to_string())
                })
                .collect()
        }
    }

    impl Drop for sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn stderr(out: &Output) -> String {
        String::from_utf8_lossy(&out.stderr).into_owned()
    }

    const Header: &str = "/**\n* Ko, (Baczynski-Bruce) 2026; Apache 2.0\n*/\n";

    /** A source file: the file header, then `lines`, each ending in a newline. */
    fn source(lines: &[&str]) -> String {
        let mut out = Header.to_string();
        for line in lines {
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    fn two_target_package(sb: &sandbox) {
        sb.write(
            "app/crepe.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n\
             [[bin]]\nname = \"app\"\nroot = \"src/main.crm\"\n\n\
             [[bin]]\nname = \"tool\"\nroot = \"src/tool.crm\"\n",
        );
        sb.write(
            "app/src/main.crm",
            &source(&[
                "module app;",
                "import { area } from \"geometry\";",
                "import { log } from \"util\";",
                "int main(void) { return 0; }",
            ]),
        );
        sb.write(
            "app/src/tool.crm",
            &source(&[
                "module tool;",
                "import { area } from \"geometry\";",
                "int main(void) { return 0; }",
            ]),
        );
        sb.write(
            "app/src/geometry.crm",
            &source(&[
                "module geometry;",
                "import { log } from \"util\";",
                "export double area(double r);",
                "double area(double r) { return r; }",
            ]),
        );
        sb.write(
            "app/src/util.crm",
            &source(&["module util;", "export void log(void);", "void log(void) {}"]),
        );
    }

    fn position(list: &[String], item: &str) -> usize {
        list.iter().position(|x| x == item).unwrap_or_else(|| panic!("`{item}` not in {list:?}"))
    }

    #[test]
    fn builds_modules_in_dependency_order_and_shares_them_between_targets() {
        let sb = sandbox::new("order");
        two_target_package(&sb);
        let out = sb.crepe_ok("app", &["build"]);
        let compiled = sb.compiled();
        ensure(compiled.len()).named("modules compiled (each compiles once)").is(4);
        ensure(position(&compiled, "src/util.crm"))
            .is_below(position(&compiled, "src/geometry.crm"));
        ensure(position(&compiled, "src/geometry.crm"))
            .is_below(position(&compiled, "src/main.crm"))
            .is_below(position(&compiled, "src/tool.crm"));
        ensure(stderr(&out))
            .contains("   Compiling geometry (src/geometry.crm)")
            .contains("    Finished debug target(s)");
        ensure(sb.path("app/build/debug/app")).is_a_file();
        ensure(sb.path("app/build/debug/tool")).is_a_file();
        ensure(sb.path("app/build/debug/modules/geometry.crmi")).is_a_file();
        ensure(sb.path("app/crepe.lock")).is_a_file();
        // Both links take the shared geometry object.
        let links: Vec<String> = sb.calls().into_iter().filter(|c| c.starts_with("-o ")).collect();
        ensure(links).has_len(2).satisfies("all take the shared geometry object", |links| {
            links.iter().all(|l| l.contains("modules/geometry.o"))
        });
        ensure(sb.calls().join("\n")).contains("-O0 -g");
    }

    #[test]
    fn rebuilds_importers_only_when_an_interface_changes() {
        let sb = sandbox::new("incremental");
        two_target_package(&sb);
        sb.crepe_ok("app", &["build"]);

        sb.crepe_ok("app", &["build"]);
        ensure(sb.compiled()).named("modules compiled when nothing changed").is_empty();

        // Body-only change: the interface hash stays the same, so importers stay fresh.
        sb.write(
            "app/src/geometry.crm",
            &source(&[
                "module geometry;",
                "import { log } from \"util\";",
                "export double area(double r);",
                "double area(double r) { return r * r; }",
            ]),
        );
        sb.crepe_ok("app", &["build"]);
        ensure(sb.compiled()).is(vec!["src/geometry.crm"]);
        ensure(sb.calls().iter().filter(|c| c.starts_with("-o ")).count())
            .named("targets relinked")
            .is(2);

        // Interface change: every importer of geometry rebuilds, util does not.
        sb.write(
            "app/src/geometry.crm",
            &source(&[
                "module geometry;",
                "import { log } from \"util\";",
                "export double area(double r, double s);",
                "double area(double r, double s) { return r * s; }",
            ]),
        );
        sb.crepe_ok("app", &["build"]);
        let mut compiled = sb.compiled();
        ensure(compiled[0].as_str()).is("src/geometry.crm");
        compiled.sort();
        ensure(compiled).is(vec!["src/geometry.crm", "src/main.crm", "src/tool.crm"]);

        // A new profile compiles everything into its own directory.
        sb.crepe_ok("app", &["build", "--release"]);
        ensure(sb.compiled()).has_len(4);
        ensure(sb.calls().join("\n")).contains("-O2");
        ensure(sb.path("app/build/release/app")).is_a_file();
    }

    #[test]
    fn builds_only_the_requested_target() {
        let sb = sandbox::new("target");
        two_target_package(&sb);
        sb.crepe_ok("app", &["build", "--target", "tool"]);
        let mut compiled = sb.compiled();
        compiled.sort();
        ensure(compiled).is(vec!["src/geometry.crm", "src/tool.crm", "src/util.crm"]);
        ensure(sb.path("app/build/debug/app")).cannot_exist();
        let out = sb.crepe("app", &["build", "--target", "nope"]);
        ensure(out).exits_with(1).stderr().contains("no target named `nope`");
    }

    #[test]
    fn links_path_dependencies_and_passes_extern_dirs() {
        let sb = sandbox::new("deps");
        sb.write("mathx/crepe.toml", "[package]\nname = \"mathx\"\nversion = \"1.0.0\"\n");
        sb.write(
            "mathx/src/lib.crm",
            &source(&[
                "module mathx;",
                "import { add } from \"core\";",
                "export int twice(int a);",
            ]),
        );
        sb.write("mathx/src/core.crm", &source(&["module core;", "export int add(int a, int b);"]));
        sb.write("mathx/src/extra.crm", &source(&["module extra;", "export int unused(void);"]));
        sb.write(
            "app/crepe.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\n",
        );
        sb.write(
            "app/src/main.crm",
            &source(&[
                "module app;",
                "import { twice } from \"mathx\";",
                "import { add } from \"mathx/core\";",
                "int main(void) { return 0; }",
            ]),
        );
        let out = sb.crepe("app", &["build"]);
        ensure(out)
            .exits_with(1)
            .stderr()
            .contains("package `mathx` is not listed under [dependencies]");

        sb.crepe_ok("app", &["add", "mathx", "--path", "../mathx"]);
        let manifest = fs::read_to_string(sb.path("app/crepe.toml")).unwrap();
        ensure(manifest).contains("[dependencies]\nmathx = { path = \"../mathx\" }\n");

        sb.crepe_ok("app", &["build"]);
        let compiled = sb.compiled();
        ensure(compiled).named("modules compiled (extra.crm is not needed)").has_len(3);
        let app_call = sb.calls().into_iter().find(|c| c.starts_with("-c src/main.crm")).unwrap();
        ensure(app_call)
            .named("compile call of main.crm")
            .contains("--extern mathx=")
            .contains("build/debug/deps/mathx/modules");
        let link = sb.calls().into_iter().find(|c| c.starts_with("-o ")).unwrap();
        ensure(link).named("link call").ends_with("build/debug/deps/mathx/libmathx.a");
        let lock = fs::read_to_string(sb.path("app/crepe.lock")).unwrap();
        ensure(lock).contains("name = \"mathx\"\nversion = \"1.0.0\"\nsource = \"path+../mathx\"");

        let tree = sb.crepe_ok("app", &["tree"]);
        let tree = String::from_utf8_lossy(&tree.stdout).into_owned();
        ensure(tree).starts_with("app v0.1.0").contains("└── mathx v1.0.0");

        sb.crepe_ok("app", &["remove", "mathx"]);
        let manifest = fs::read_to_string(sb.path("app/crepe.toml")).unwrap();
        ensure(manifest).cannot_contain("mathx");
    }

    #[test]
    fn reports_cycles_and_compile_errors() {
        let sb = sandbox::new("errors");
        sb.write("app/crepe.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
        sb.write("app/src/main.crm", &source(&["module app;", "import { a } from \"a\";"]));
        sb.write("app/src/a.crm", &source(&["module a;", "import { b } from \"b\";"]));
        sb.write("app/src/b.crm", &source(&["module b;", "import { a } from \"a\";"]));
        let out = sb.crepe("app/src", &["build"]);
        ensure(out)
            .exits_with(1)
            .stderr()
            .contains("error: package `app`: import cycle: a -> b -> a");
        ensure(sb.calls()).named("crust calls").is_empty();

        sb.write("app/src/b.crm", &source(&["module b;", "int COMPILE_ERROR;"]));
        let out = sb.crepe("app", &["build"]);
        ensure(out)
            .exits_with(1)
            .stderr()
            .named("diagnostics pass through")
            .contains("src/b.crm:1: error: COMPILE_ERROR")
            .contains("error: could not compile `b`");
        ensure(sb.path("app/build/debug/modules/b.state")).cannot_exist();
    }

    #[test]
    fn new_package_builds_runs_and_tests() {
        let sb = sandbox::new("new");
        sb.crepe_ok("", &["new", "hello"]);
        let main = fs::read_to_string(sb.path("hello/src/main.crm")).unwrap();
        ensure(main)
            .starts_with(source(&["module hello;"]))
            .contains("printf(\"Hello from hello!\\n\");");
        let toml = fs::read_to_string(sb.path("hello/crepe.toml")).unwrap();
        ensure(toml).starts_with("# Ko, (Baczynski-Bruce) 2026; Apache 2.0\n");
        let ignore = fs::read_to_string(sb.path("hello/.gitignore")).unwrap();
        ensure(ignore).is("# Ko, (Baczynski-Bruce) 2026; Apache 2.0\n\nbuild/\n");

        let out = sb.crepe_ok("hello", &["run", "--", "a", "b"]);
        ensure(out).stdout().contains("build/debug/hello a b");

        let bad = sb.crepe("", &["new", "Hello-World"]);
        ensure(bad).exits_with(1).stderr().contains("try `hello_world`");

        let reg = sb.crepe("hello", &["add", "mathx@1.2"]);
        ensure(reg)
            .exits_with(1)
            .stderr()
            .contains("registry dependencies are not supported in 0.1");

        sb.write(
            "hello/tests/pass.crm",
            &source(&["module pass_test;", "int main(void) { return 0; }"]),
        );
        sb.write("hello/tests/fail.crm", &source(&["module fail_test;", "int TEST_FAIL;"]));
        let mut toml = fs::read_to_string(sb.path("hello/crepe.toml")).unwrap();
        toml.push_str("\n[[test]]\nname = \"pass\"\nroot = \"tests/pass.crm\"\n");
        toml.push_str("\n[[test]]\nname = \"fail\"\nroot = \"tests/fail.crm\"\n");
        sb.write("hello/crepe.toml", &toml);
        let out = sb.crepe("hello", &["test"]);
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        ensure(out).exits_with(1);
        ensure(stdout).contains("test pass ... ok").contains("test fail ... FAILED");

        let check = sb.crepe_ok("hello", &["check"]);
        ensure(check).stderr().contains("Finished");
        ensure(sb.calls().iter().filter(|c| c.starts_with("-o ")).count())
            .named("link calls of `crepe check`")
            .is(0);

        sb.crepe_ok("hello", &["clean"]);
        ensure(sb.path("hello/build")).cannot_exist();

        let usage = sb.crepe("hello", &["build", "--bogus"]);
        ensure(usage).exits_with(2);
        let version = sb.crepe_ok("", &["--version"]);
        ensure(version).stdout().is(format!("crepe {}\n", env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn builds_old_school_sources_and_library_headers() {
        let sb = sandbox::new("legacy");
        sb.write(
            "geo/crepe.toml",
            "[package]\nname = \"geo\"\nversion = \"0.1.0\"\n\n\
             [[lib]]\nname = \"geo\"\nroot = \"src/lib.crm\"\n\n\
             [[bin]]\nname = \"legacy-tool\"\nsources = [\"src/tool/main.cr\"]\n\
             include = [\"src/tool\"]\n",
        );
        sb.write(
            "geo/src/lib.crm",
            &source(&[
                "module geo;",
                "import { sq } from \"shapes\";",
                "export int geo_area(int r);",
            ]),
        );
        sb.write("geo/src/shapes.crm", &source(&["module shapes;", "export int sq(int x);"]));
        sb.write(
            "geo/src/tool/main.cr",
            &source(&[
                "#include \"parse.hr\"",
                "import { sq } from \"shapes\";",
                "int main(void) { return 0; }",
            ]),
        );
        sb.write("geo/src/tool/parse.hr", &source(&["int parse(void);"]));
        sb.crepe_ok("geo", &["build"]);
        ensure(sb.path("geo/build/debug/libgeo.a")).is_a_file();
        ensure(sb.path("geo/build/debug/include/geo.h")).is_a_file();
        ensure(sb.path("geo/build/debug/legacy-tool")).is_a_file();
        let cr = sb.calls().into_iter().find(|c| c.starts_with("-c src/tool/main.cr")).unwrap();
        ensure(cr).named("compile call of main.cr").contains("-I src/tool");

        // `.cr` sources have no interface: they rebuild every time; modules stay fresh.
        sb.crepe_ok("geo", &["build"]);
        ensure(sb.compiled()).is(vec!["src/tool/main.cr"]);

        // Editing an included `.hr` header rebuilds the modules that include it.
        sb.write(
            "geo/src/shapes.crm",
            &source(&["module shapes;", "#include \"tool/parse.hr\"", "export int sq(int x);"]),
        );
        sb.crepe_ok("geo", &["build"]);
        sb.write("geo/src/tool/parse.hr", &source(&["int parse(int);"]));
        sb.crepe_ok("geo", &["build"]);
        ensure(sb.compiled()).is(vec!["src/shapes.crm", "src/tool/main.cr"]);
        ensure(sb.path("geo/build/debug/modules/shapes.state")).is_a_file();
    }
    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(["-c", "user.name=crepe", "-c", "user.email=crepe@example.invalid"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        let text = ensure(out).named(format!("git {args:?}")).succeeds().stdout().into_value();
        text.trim().to_string()
    }

    #[test]
    fn locks_and_updates_git_dependencies() {
        if Command::new("git").arg("--version").output().is_err() {
            eprintln!("skipping: git is not installed");
            return;
        }
        let sb = sandbox::new("git");
        sb.write("gdep/crepe.toml", "[package]\nname = \"gdep\"\nversion = \"0.2.0\"\n");
        sb.write("gdep/src/lib.crm", &source(&["module gdep;", "export int g(void);"]));
        let repo = sb.path("gdep");
        git(&repo, &["init", "--quiet", "--initial-branch=main"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "--quiet", "-m", "one"]);
        let first = git(&repo, &["rev-parse", "HEAD"]);

        sb.crepe_ok("", &["new", "app"]);
        let url = format!("file://{}", repo.display());
        sb.crepe_ok("app", &["add", "gdep", "--git", &url, "--rev", "main"]);
        sb.write(
            "app/src/main.crm",
            &source(&[
                "module app;",
                "import { g } from \"gdep\";",
                "int main(void) { return 0; }",
            ]),
        );
        sb.crepe_ok("app", &["build"]);
        let lock = fs::read_to_string(sb.path("app/crepe.lock")).unwrap();
        ensure(lock).contains(format!("rev = \"main\"\ncommit = \"{first}\""));
        ensure(sb.path("home/git").read_dir().unwrap().count()).named("cached clones").is(1);

        // A new upstream commit (body-only) is ignored until `crepe update`.
        sb.write("gdep/src/lib.crm", &source(&["module gdep;", "export int g(void);", "// two"]));
        git(&repo, &["commit", "--quiet", "-am", "two"]);
        let second = git(&repo, &["rev-parse", "HEAD"]);
        sb.crepe_ok("app", &["build"]);
        ensure(sb.compiled()).named("modules compiled after an ignored upstream commit").is_empty();

        sb.crepe_ok("app", &["update"]);
        let lock = fs::read_to_string(sb.path("app/crepe.lock")).unwrap();
        ensure(lock).contains(second);
        sb.crepe_ok("app", &["build"]);
        let compiled = sb.compiled();
        ensure(compiled.len()).named("modules compiled (only the dependency)").is(1);
        ensure(compiled[0].as_str()).ends_with("src/lib.crm");
    }
}
