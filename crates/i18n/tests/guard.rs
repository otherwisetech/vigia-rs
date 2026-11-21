//! Source guards for the i18n rule (see CLAUDE.md):
//! 1. no hard-coded user-facing text passed to UI sinks in the app, the CLI or the core;
//! 2. every key used in code exists in the default catalog, and every catalog key is used.
//!
//! Escape hatch for a literal that really is not translatable (product name, symbols):
//! end its line with `// i18n-ok`.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const SCANNED: &[&str] = &["../app/src", "../core/src", "../cli/src", "src"];

/// Calls whose literal arguments end up on screen: methods (`.child("…")`), free helpers
/// (`section_title(t, "…")`), `Type::fn` constructors and the `#[error("…")]` attribute.
const CALL_SINKS: &[&str] = &[
    "child", "label", "tooltip", "placeholder", "set_placeholder", "set_window_title", "title", "prompt", "section_title", "field", "card", "empty_state", "chip", "cell", "error", "println",
];
const PATH_SINKS: &[(&str, &str)] = &[("SharedString", "from"), ("Tooltip", "new"), ("PopupMenuItem", "new"), ("MenuItem", "action"), ("Label", "new"), ("Menu", "new")];
/// Struct fields that hold displayed text (`title: Some("…".into())`).
const FIELD_SINKS: &[&str] = &["title", "name", "prompt", "placeholder", "label", "tooltip", "message", "msg"];
/// Calls that just wrap the literal; the sink is looked up around them.
const TRANSPARENT: &[&str] = &["Some", "Ok", "Err", "format", "from", "into", "to_string", "to_owned"];

/// Keys built at runtime (`format!("app.kind.{}", …)`); every key under them counts as used.
/// A new dynamic prefix must be added here on purpose.
const DYNAMIC_PREFIXES: &[&str] = &[];

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Str(String),
    Ident(String),
    Punct(char),
}

struct Token {
    tok: Tok,
    line: usize,
}

struct Source {
    tokens: Vec<Token>,
    ok_lines: BTreeSet<usize>,
}

/// Minimal Rust lexer: enough to find string literals, idents and brackets.
fn lex(src: &str) -> Source {
    let c: Vec<char> = src.chars().collect();
    let (mut i, mut line) = (0, 1);
    let mut tokens = Vec::new();
    let mut ok_lines = BTreeSet::new();
    while i < c.len() {
        let ch = c[i];
        if ch == '\n' {
            line += 1;
            i += 1;
        } else if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && c.get(i + 1) == Some(&'/') {
            let start = i;
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            if c[start..i].iter().collect::<String>().contains("i18n-ok") {
                ok_lines.insert(line);
            }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                line += (c[i] == '\n') as usize;
                i += 1;
            }
            i += 2;
        } else if ch == '"' || (ch == 'r' && matches!(c.get(i + 1), Some('"' | '#'))) || (ch == 'b' && c.get(i + 1) == Some(&'"')) {
            let start_line = line;
            let mut s = String::new();
            if ch == 'r' {
                i += 1;
                let mut hashes = 0;
                while c[i] == '#' {
                    hashes += 1;
                    i += 1;
                }
                i += 1;
                loop {
                    if c[i] == '"' && c[i + 1..].iter().take(hashes).filter(|x| **x == '#').count() == hashes {
                        i += 1 + hashes;
                        break;
                    }
                    line += (c[i] == '\n') as usize;
                    s.push(c[i]);
                    i += 1;
                }
            } else {
                i += if ch == 'b' { 2 } else { 1 };
                while c[i] != '"' {
                    if c[i] == '\\' {
                        i += 1;
                        match c[i] {
                            'n' | 't' | 'r' | '0' => s.push(' '),
                            'u' => {
                                while c[i] != '}' {
                                    i += 1;
                                }
                                s.push(' ');
                            }
                            '\n' => {
                                line += 1;
                                i += 1;
                                while c[i].is_whitespace() {
                                    line += (c[i] == '\n') as usize;
                                    i += 1;
                                }
                                continue;
                            }
                            x => s.push(x),
                        }
                        i += 1;
                        continue;
                    }
                    line += (c[i] == '\n') as usize;
                    s.push(c[i]);
                    i += 1;
                }
                i += 1;
            }
            tokens.push(Token { tok: Tok::Str(s), line: start_line });
        } else if ch == '\'' {
            // char literal or lifetime
            if c.get(i + 1) == Some(&'\\') {
                i += 2;
                while c[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if c.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
        } else if ch.is_alphanumeric() || ch == '_' {
            let start = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            tokens.push(Token { tok: Tok::Ident(c[start..i].iter().collect()), line });
        } else {
            tokens.push(Token { tok: Tok::Punct(ch), line });
            i += 1;
        }
    }
    Source { tokens: strip_tests(tokens), ok_lines }
}

fn is_p(t: Option<&Token>, ch: char) -> bool {
    matches!(t, Some(Token { tok: Tok::Punct(p), .. }) if *p == ch)
}

fn ident(t: Option<&Token>) -> Option<&str> {
    match t {
        Some(Token { tok: Tok::Ident(s), .. }) => Some(s),
        _ => None,
    }
}

/// Drops items marked `#[cfg(test)]` or `#[test]` (up to their closing brace or `;`).
fn strip_tests(toks: Vec<Token>) -> Vec<Token> {
    let text = |t: &Token| match &t.tok {
        Tok::Ident(s) => s.clone(),
        Tok::Punct(p) => p.to_string(),
        Tok::Str(_) => "\"".into(),
    };
    let mut keep = vec![true; toks.len()];
    let mut i = 0;
    while i < toks.len() {
        let a: String = toks[i..toks.len().min(i + 7)].iter().map(text).collect();
        if !(a.starts_with("#[cfg(test)]") || a.starts_with("#[test]")) {
            i += 1;
            continue;
        }
        let mut depth = 0;
        while i < toks.len() {
            keep[i] = false;
            i += 1;
            match toks[i - 1].tok {
                Tok::Punct('{') => depth += 1,
                Tok::Punct('}') => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Tok::Punct(';') if depth == 0 => break,
                _ => {}
            }
        }
    }
    toks.into_iter().zip(keep).filter_map(|(t, k)| k.then_some(t)).collect()
}

fn files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    for d in SCANNED {
        walk(&root.join(d), &mut out);
    }
    out.sort();
    out
}

fn sources() -> Vec<(String, Source)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    files().into_iter().map(|p| (p.canonicalize().unwrap().strip_prefix(root.canonicalize().unwrap()).unwrap().display().to_string(), lex(&std::fs::read_to_string(&p).unwrap()))).collect()
}

/// Text with at least two letters in a row once `{placeholders}` are removed.
fn has_words(s: &str) -> bool {
    let mut depth = 0;
    let mut run = 0;
    for ch in s.chars() {
        match ch {
            '{' => depth += 1,
            '}' => depth -= 1,
            c if depth == 0 && c.is_alphabetic() => {
                run += 1;
                if run >= 2 {
                    return true;
                }
                continue;
            }
            _ => {}
        }
        run = 0;
    }
    false
}

/// `HUSB`, `_UID`: `node.child("NAME")` reads a GEDCOM tag, it doesn't render it.
fn is_gedcom_tag(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn looks_like_key(s: &str, modules: &BTreeSet<String>) -> bool {
    s.split_once('.').is_some_and(|(m, rest)| modules.contains(m) && !rest.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.' || c == '{' || c == '}'))
}

/// Name of the sink the literal at `i` flows into, if any.
fn sink(t: &[Token], i: usize) -> Option<String> {
    let mut start = i;
    loop {
        // the expression may continue with `.into()` / `.to_string()`: irrelevant for the sink
        let before = start.checked_sub(1).map(|k| &t[k]);
        if is_p(before, ':') && !is_p(start.checked_sub(2).map(|k| &t[k]), ':') {
            let f = ident(start.checked_sub(2).map(|k| &t[k]))?;
            return FIELD_SINKS.contains(&f).then(|| format!("{f}:"));
        }
        // find the unmatched `(` enclosing `start`
        let mut depth = 0;
        let mut k = start;
        let open = loop {
            k = k.checked_sub(1)?;
            match t[k].tok {
                Tok::Punct(')' | ']' | '}') => depth += 1,
                Tok::Punct('(') if depth == 0 => break k,
                Tok::Punct('(' | '[') => depth -= 1,
                Tok::Punct('{' | ';') if depth == 0 => return None,
                Tok::Punct('{') => depth -= 1,
                _ => {}
            }
        };
        let mut callee = open.checked_sub(1)?;
        if is_p(Some(&t[callee]), '!') {
            callee = callee.checked_sub(1)?;
        }
        let name = ident(Some(&t[callee]))?;
        if TRANSPARENT.contains(&name) {
            start = callee;
            // walk back over a path like `SharedString::from` / `String::from`
            while is_p(start.checked_sub(1).map(|k| &t[k]), ':') && is_p(start.checked_sub(2).map(|k| &t[k]), ':') {
                let ty = ident(start.checked_sub(3).map(|k| &t[k]));
                if let Some(ty) = ty
                    && PATH_SINKS.contains(&(ty, name))
                {
                    return Some(format!("{ty}::{name}"));
                }
                start -= 3;
            }
            continue;
        }
        if is_p(callee.checked_sub(1).map(|k| &t[k]), ':')
            && let Some(ty) = ident(callee.checked_sub(3).map(|k| &t[k]))
        {
            return PATH_SINKS.contains(&(ty, name)).then(|| format!("{ty}::{name}"));
        }
        return CALL_SINKS.contains(&name).then(|| name.to_string());
    }
}

fn modules() -> BTreeSet<String> {
    i18n::keys(i18n::DEFAULT).iter().filter_map(|k| k.split('.').next()).map(str::to_string).collect()
}

#[test]
fn no_hardcoded_ui_text() {
    let modules = modules();
    let mut problems = Vec::new();
    for (file, src) in sources() {
        for (i, tk) in src.tokens.iter().enumerate() {
            let Tok::Str(s) = &tk.tok else { continue };
            if !has_words(s) || looks_like_key(s, &modules) || is_gedcom_tag(s) || src.ok_lines.contains(&tk.line) {
                continue;
            }
            if let Some(name) = sink(&src.tokens, i) {
                problems.push(format!("{file}:{}: {s:?} → {name}", tk.line));
            }
        }
    }
    assert!(problems.is_empty(), "user-facing text must go through i18n::t! (or end the line with `// i18n-ok`):\n{}", problems.join("\n"));
}

#[test]
fn keys_used_exist_and_catalog_keys_are_used() {
    let modules = modules();
    let catalog: BTreeSet<&str> = i18n::keys(i18n::DEFAULT).into_iter().collect();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut problems = Vec::new();
    let mut dynamic_seen = BTreeSet::new();
    for (file, src) in sources() {
        for tk in &src.tokens {
            let Tok::Str(s) = &tk.tok else { continue };
            if !looks_like_key(s, &modules) {
                continue;
            }
            if let Some((prefix, _)) = s.split_once('{') {
                if DYNAMIC_PREFIXES.contains(&prefix) {
                    dynamic_seen.insert(prefix.to_string());
                } else {
                    problems.push(format!("{file}:{}: dynamic key {s:?} — use static keys or add its prefix to DYNAMIC_PREFIXES", tk.line));
                }
                continue;
            }
            // tn! expands to .one/.other, tv! to .vowel
            let family: Vec<String> = [String::new(), ".one".into(), ".other".into(), ".vowel".into()].iter().map(|sfx| format!("{s}{sfx}")).collect();
            if !family.iter().any(|k| catalog.contains(k.as_str())) {
                problems.push(format!("{file}:{}: key {s:?} missing from locales/{}/", tk.line, i18n::DEFAULT));
            }
            used.extend(family);
        }
    }
    for p in DYNAMIC_PREFIXES {
        if !dynamic_seen.contains(*p) {
            problems.push(format!("DYNAMIC_PREFIXES has {p:?} but no code builds keys with it"));
        }
    }
    for k in &catalog {
        let dynamic = DYNAMIC_PREFIXES.iter().any(|p| k.starts_with(p));
        if !dynamic && !used.contains(*k) {
            problems.push(format!("unused catalog key {k}"));
        }
    }
    assert!(problems.is_empty(), "i18n keys out of sync with code:\n{}", problems.join("\n"));
}

#[test]
fn guard_detects_sinks() {
    let src = lex("fn f() { div().child(\"Hola mundo\"); x.label(format!(\"Total {n}\").into()); let t = Tooltip::new(\"Abrir\"); o = W { title: Some(\"gen\".into()) }; div().id(\"raw-close\"); div().child(\"⌘K\"); n.child(\"HUSB\");\n div().child(\"ok\") // i18n-ok\n }\n#[cfg(test)]\nmod tests { fn g() { div().child(\"test only\"); } }");
    let found: Vec<String> = src.tokens.iter().enumerate().filter_map(|(i, t)| match &t.tok {
        Tok::Str(s) if has_words(s) && !is_gedcom_tag(s) && !src.ok_lines.contains(&t.line) => sink(&src.tokens, i).map(|n| format!("{s}:{n}")),
        _ => None,
    }).collect();
    assert_eq!(found, vec!["Hola mundo:child", "Total {n}:label", "Abrir:Tooltip::new", "gen:title:"]);
}
