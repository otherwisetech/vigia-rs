//! Translations for every user-facing string of Vigía (app, CLI and the core's messages).
//!
//! Catalogs live in `locales/<lang>/<module>.ftl` (one file per module so contributors don't
//! collide) and are embedded at build time. Format, one entry per line:
//!
//! ```text
//! # comment
//! app.item.new = Nuevo elemento…
//! core.ics.expires = Vence: {title}
//! core.rec.years.one = Cada año
//! core.rec.years.other = Cada {n} años
//! ```
//!
//! Rule: every key must exist in every language (`tests::all_locales_have_the_same_keys`);
//! `tests/guard.rs` checks that code and catalogs agree and that UI text isn't hard-coded.
//! Lookup falls back to the default language, then to the key itself.
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

include!(concat!(env!("OUT_DIR"), "/catalog.rs"));

pub const DEFAULT: &str = "es";

/// (code, native name) of every bundled language, sorted by code. The name is the
/// language's own `common.language_name`.
pub fn available() -> Vec<(&'static str, &'static str)> {
    CATALOG.iter().map(|(code, _)| (*code, tables().get(code).and_then(|m| m.get("common.language_name")).copied().unwrap_or(code))).collect()
}

type Table = HashMap<&'static str, HashMap<&'static str, &'static str>>;

fn tables() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| CATALOG.iter().map(|(lang, files)| (*lang, files.iter().flat_map(|(_, src)| parse(src)).collect())).collect())
}

fn parse(src: &'static str) -> impl Iterator<Item = (&'static str, &'static str)> {
    src.lines().filter_map(|l| {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') {
            return None;
        }
        let (k, v) = l.split_once('=')?;
        Some((k.trim(), v.trim()))
    })
}

static CURRENT: RwLock<&'static str> = RwLock::new(DEFAULT);

pub fn locale() -> &'static str {
    *CURRENT.read().unwrap()
}

/// Switch language; unknown codes are ignored. Returns whether it changed.
pub fn set_locale(code: &str) -> bool {
    let Some((code, _)) = CATALOG.iter().find(|(c, _)| *c == code) else { return false };
    let mut cur = CURRENT.write().unwrap();
    let changed = *cur != *code;
    *cur = code;
    changed
}

/// Best bundled match for a system locale like "en-US" / "pt_BR"; None if unsupported.
pub fn match_system(tag: &str) -> Option<&'static str> {
    let base = tag.split(['-', '_', '.']).next()?.to_lowercase();
    CATALOG.iter().map(|(c, _)| *c).find(|c| *c == base)
}

/// `key` in `lang` (no arguments), else the default language, else the key; the current
/// language doesn't change. For texts other programs show in the user's language (manifests).
pub fn tr_in(lang: &str, key: &str) -> String {
    let t = tables();
    t.get(lang).and_then(|m| m.get(key)).or_else(|| t.get(DEFAULT).and_then(|m| m.get(key))).copied().unwrap_or(key).to_string()
}

fn raw(key: &str) -> Option<&'static str> {
    let t = tables();
    t.get(locale()).and_then(|m| m.get(key)).or_else(|| t.get(DEFAULT).and_then(|m| m.get(key))).copied()
}

/// Translate with `{name}` placeholders. Unknown key → the key itself (visible, never panics).
pub fn tr(key: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut s = raw(key).unwrap_or(key).replace("\\n", "\n");
    for (name, val) in args {
        s = s.replace(&format!("{{{name}}}"), &val.to_string());
    }
    s
}

/// Whether `n` takes the singular form in the current language (French counts 0 as singular).
fn is_one(n: i64) -> bool {
    match locale() {
        "fr" => n == 0 || n == 1,
        _ => n == 1,
    }
}

/// Plural: looks up `key.one` for a singular count, else `key.other`; `{n}` is filled in.
pub fn tr_n(key: &str, n: i64, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let k = format!("{key}.{}", if is_one(n) { "one" } else { "other" });
    let mut all: Vec<(&str, &dyn std::fmt::Display)> = vec![("n", &n)];
    all.extend_from_slice(args);
    tr(&k, &all)
}

/// Like [`tr`], but uses `key.vowel` when `word` starts with a vowel sound, for languages that
/// elide before it (fr «fils de Luc» / «fils d'Anne»). Every language defines both keys.
pub fn tr_v(key: &str, word: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let vowel = word.trim_start().chars().next().is_some_and(|c| {
        let c = c.to_lowercase().next().unwrap_or(c);
        "aeiouyhàâäáéèêëíìîïóòôöúùûüÿæœ".contains(c)
    });
    if vowel { tr(&format!("{key}.vowel"), args) } else { tr(key, args) }
}

/// Every key of a bundled language, sorted (used by the guard tests).
pub fn keys(lang: &str) -> Vec<&'static str> {
    let mut v: Vec<_> = tables().get(lang).map(|m| m.keys().copied().collect()).unwrap_or_default();
    v.sort_unstable();
    v
}

/// `t!("key")`, `t!("key", name = value, …)`
#[macro_export]
macro_rules! t {
    ($key:expr) => { $crate::tr($key, &[]) };
    ($key:expr, $($name:ident = $val:expr),+ $(,)?) => { $crate::tr($key, &[$((stringify!($name), &$val as &dyn ::std::fmt::Display)),+]) };
}

/// `tv!("key", word, name = value, …)` — see [`tr_v`].
#[macro_export]
macro_rules! tv {
    ($key:expr, $word:expr) => { $crate::tr_v($key, &$word, &[]) };
    ($key:expr, $word:expr, $($name:ident = $val:expr),+ $(,)?) => { $crate::tr_v($key, &$word, &[$((stringify!($name), &$val as &dyn ::std::fmt::Display)),+]) };
}

/// `tn!("key", count)`, `tn!("key", count, name = value, …)` — see [`tr_n`].
#[macro_export]
macro_rules! tn {
    ($key:expr, $n:expr) => { $crate::tr_n($key, ($n) as i64, &[]) };
    ($key:expr, $n:expr, $($name:ident = $val:expr),+ $(,)?) => { $crate::tr_n($key, ($n) as i64, &[$((stringify!($name), &$val as &dyn ::std::fmt::Display)),+]) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn all_locales_have_the_same_keys() {
        let t = tables();
        let reference: BTreeSet<_> = t[DEFAULT].keys().copied().collect();
        let mut problems = Vec::new();
        for (lang, m) in t {
            let keys: BTreeSet<_> = m.keys().copied().collect();
            for k in reference.difference(&keys) {
                problems.push(format!("{lang}: missing {k}"));
            }
            for k in keys.difference(&reference) {
                problems.push(format!("{lang}: extra {k} (not in {DEFAULT})"));
            }
            for (k, v) in m {
                if v.is_empty() {
                    problems.push(format!("{lang}: empty {k}"));
                }
            }
        }
        assert!(problems.is_empty(), "translation catalogs out of sync:\n{}", problems.join("\n"));
    }

    #[test]
    fn placeholders_match_across_locales() {
        let t = tables();
        let ph = |v: &str| v.split('{').skip(1).filter_map(|s| s.split_once('}').map(|(a, _)| a.to_string())).collect::<BTreeSet<_>>();
        for (lang, m) in t {
            for (k, v) in m {
                if let Some(d) = t[DEFAULT].get(k) {
                    assert_eq!(ph(v), ph(d), "{lang}:{k} placeholders differ from {DEFAULT}");
                }
            }
        }
    }

    #[test]
    fn lookup_args_plural_fallback() {
        // single test touches the global locale to avoid races between tests
        set_locale("es");
        assert_eq!(tn!("core.rec.years", 1), "Cada año");
        assert_eq!(tn!("core.rec.years", 3), "Cada 3 años");
        assert_eq!(t!("core.ics.expires", title = "SOAT"), "Vence: SOAT");
        assert!(set_locale("fr"));
        assert_eq!(tn!("core.rec.years", 0), "Chaque année");
        assert!(set_locale("en"));
        assert_eq!(tn!("core.rec.years", 1), "Every year");
        assert_eq!(t!("no.such.key"), "no.such.key");
        assert!(!set_locale("xx"));
        assert_eq!(match_system("en-US"), Some("en"));
        assert_eq!(match_system("pt_BR"), Some("pt"));
        let names: Vec<_> = available().into_iter().map(|(_, n)| n).collect();
        assert_eq!(names, ["English", "Español", "Français", "Português"]);
        set_locale("es");
    }
}
