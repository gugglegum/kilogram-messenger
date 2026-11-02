//! Bounded public reads, recursive duplicate rejection and explicit source scope.
use super::*;
use serde::de::{DeserializeOwned, MapAccess, SeqAccess, Visitor};
use std::io::Read;
#[derive(Debug)]
struct Strict(Value);
impl<'de> serde::Deserialize<'de> for Strict {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Strict;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("strict bounded JSON")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Strict, E> {
                Ok(Strict(json!(v)))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::Number(
                    serde_json::Number::from_f64(v).ok_or_else(|| E::custom("finite number"))?,
                )))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Strict, E> {
                self.visit_string(v.into())
            }
            fn visit_string<E: serde::de::Error>(
                self,
                v: String,
            ) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::String(v)))
            }
            fn visit_none<E: serde::de::Error>(self) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Strict, E> {
                self.visit_none()
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Strict, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = a.next_element::<Strict>()? {
                    out.push(v.0);
                }
                Ok(Strict(Value::Array(out)))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Strict, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some((k, v)) = a.next_entry::<String, Strict>()? {
                    if out.insert(k, v.0).is_some() {
                        return Err(serde::de::Error::custom("duplicate JSON key"));
                    }
                }
                Ok(Strict(Value::Object(out)))
            }
        }
        d.deserialize_any(V)
    }
}
pub(super) fn json<T: DeserializeOwned>(raw: &[u8], cap: usize) -> Result<T> {
    ensure!(
        raw.len() <= cap && !raw.starts_with(&[0xef, 0xbb, 0xbf]),
        "public JSON bound/BOM"
    );
    let value: Strict =
        serde_json::from_slice(raw).map_err(|_| anyhow::anyhow!("strict public JSON"))?;
    serde_json::from_value(value.0).map_err(|_| anyhow::anyhow!("closed public schema"))
}
pub(super) fn sha(raw: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    state::hex(&Sha256::digest(raw))
}
fn fingerprint(
    m: &std::fs::Metadata,
) -> Result<(u64, std::time::SystemTime, std::time::SystemTime)> {
    Ok((m.len(), m.created()?, m.modified()?))
}
pub(super) fn regular(path: &Path) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    ensure!(!m.file_type().is_symlink(), "no symlink in public path");
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(m.file_attributes() & 0x400 == 0, "no reparse point");
    }
    Ok(())
}
pub(super) fn path(path: &Path, allow_missing: bool) -> Result<PathBuf> {
    ensure!(
        path.is_absolute() && path.as_os_str().len() <= 1024,
        "absolute bounded public path"
    );
    ensure!(
        !path.components().any(|c| matches!(
            c,
            std::path::Component::ParentDir | std::path::Component::CurDir
        )),
        "no path escape/alias components"
    );
    let mut probe = path.to_path_buf();
    while !probe.exists() {
        if std::fs::symlink_metadata(&probe).is_ok() {
            regular(&probe)?;
        }
        ensure!(allow_missing, "required public path");
        ensure!(probe.pop(), "existing public ancestor");
    }
    ensure!(
        probe == path || probe.is_dir(),
        "missing suffix requires a directory ancestor"
    );
    for ancestor in probe.ancestors() {
        regular(ancestor)?;
    }
    let base = probe.canonicalize()?;
    if probe == path {
        Ok(base)
    } else {
        Ok(base.join(path.strip_prefix(probe)?))
    }
}
pub(super) fn same(a: &Path, b: &Path) -> Result<()> {
    ensure!(
        path(a, true)? == path(b, true)?,
        "exact canonical path role"
    );
    Ok(())
}
pub(super) fn key(path: &Path) -> Result<String> {
    Ok(self::path(path, true)?.to_string_lossy().to_lowercase())
}
pub(super) fn read(path: &Path, cap: usize) -> Result<Vec<u8>> {
    let canonical = self::path(path, false)?;
    let before = std::fs::metadata(path)?;
    ensure!(
        before.is_file() && before.len() <= cap as u64,
        "bounded regular public file"
    );
    let file = std::fs::File::open(path)?;
    let opened = file.metadata()?;
    ensure!(
        fingerprint(&before)? == fingerprint(&opened)?,
        "read-time public file identity"
    );
    let mut raw = Vec::new();
    (&file).take(cap as u64 + 1).read_to_end(&mut raw)?;
    ensure!(
        raw.len() <= cap && raw.len() as u64 == opened.len(),
        "bounded stable public read"
    );
    ensure!(
        self::path(path, false)? == canonical
            && fingerprint(&file.metadata()?)? == fingerprint(&opened)?
            && fingerprint(&std::fs::metadata(path)?)? == fingerprint(&opened)?,
        "post-read public file identity"
    );
    Ok(raw)
}
pub(super) fn digest(v: &str, n: usize) -> Result<()> {
    ensure!(
        v.len() == n
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "canonical lower hex digest"
    );
    Ok(())
}
pub(super) fn pin(p: &Pin, cap: usize) -> Result<Vec<u8>> {
    digest(&p.sha256, 64)?;
    let raw = read(&p.path, cap)?;
    ensure!(
        raw.len() as u64 == p.bytes && sha(&raw) == p.sha256,
        "actual pinned public bytes"
    );
    Ok(raw)
}
pub(super) fn document<T: DeserializeOwned>(p: &Pin, cap: usize) -> Result<T> {
    json(&pin(p, cap)?, cap)
}
pub(super) const HELPERS: [&str; 17] = [
    "scripts/run-cargo-tests-stable.ps1",
    "scripts/cargo-frozen-artifacts.ps1",
    "scripts/cargo-resource-policy.ps1",
    "scripts/migration-s5-l1-deadline.ps1",
    "scripts/migration-s53-metadata.ps1",
    "scripts/migration-s53-runtime-dependencies.ps1",
    "scripts/migration-s5-l1-test-result.ps1",
    "scripts/invoke-migration-s54-whole.ps1",
    "scripts/test-migration-s54-admission.ps1",
    "scripts/migration-s54-outer.ps1",
    "scripts/test-migration-s54-whole.ps1",
    "apps/kilogram-cli/tests/support/migration_s54_outer_fixture.ps1",
    "scripts/migration-s54-preparation.ps1",
    "scripts/test-migration-s54-repair.ps1",
    "scripts/test-migration-s54-feasibility.ps1",
    "scripts/test-migration-s54-preparation-order.ps1",
    "scripts/test-migration-s54-diagnostics.ps1",
];
pub(super) const EMBEDDED: [&str; 3] = [
    "apps/kilogram-cli/tests/support/migration_s54_capture_fixture.ps1",
    "apps/kilogram-cli/tests/support/migration_s54_memory_fixture.ps1",
    "ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-M1.candidate.json",
];
pub(super) fn relevant(s: &str) -> bool {
    let p = Path::new(s);
    let code = s.starts_with("apps/") || s.starts_with("crates/");
    (code
        && (p.file_name().is_some_and(|s| s == "Cargo.toml")
            || p.extension()
                .is_some_and(|s| ["rs", "c", "h", "cpp"].iter().any(|v| s == *v))))
        || [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "rust-toolchain",
        ]
        .contains(&s)
        || HELPERS.contains(&s)
        || EMBEDDED.contains(&s)
}
pub(super) fn sources(root: &Path) -> Result<Vec<PathBuf>> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
        ensure!(out.len() < 4096, "source-set bound");
        for entry in std::fs::read_dir(dir)? {
            let p = entry?.path();
            regular(&p)?;
            if p.is_dir() {
                if !["target", ".tmp", ".git"]
                    .iter()
                    .any(|n| p.file_name().is_some_and(|v| v == *n))
                {
                    walk(root, &p, out)?;
                }
            } else {
                let rel = p.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
                if relevant(&rel) {
                    out.push(p);
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    for name in ["apps", "crates"] {
        walk(root, &root.join(name), &mut out)?;
    }
    for name in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "rust-toolchain",
    ] {
        if root.join(name).exists() {
            out.push(root.join(name));
        }
    }
    for name in HELPERS {
        ensure!(root.join(name).is_file(), "required S54 helper source");
        if !out.contains(&root.join(name)) {
            out.push(root.join(name));
        }
    }
    // The two scripts are found by the apps walk; the one exact historical
    // public JSON is added directly, never by walking ai-docs or history.
    for name in EMBEDDED {
        ensure!(
            root.join(name).is_file(),
            "required embedded compiler input"
        );
        if !out.contains(&root.join(name)) {
            out.push(root.join(name));
        }
    }
    out.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    let mut seen = BTreeSet::new();
    for p in &out {
        ensure!(seen.insert(key(p)?), "source aliases");
    }
    Ok(out)
}
pub(super) fn dep_sources(root: &Path, dep: &Pin) -> Result<Vec<PathBuf>> {
    let raw = pin(dep, MIB)?;
    ensure!(!raw.starts_with(&[0xef, 0xbb, 0xbf]), "dep-info BOM");
    let text = std::str::from_utf8(&raw)?;
    let (target, rest) = text
        .lines()
        .next()
        .context("dep-info first rule")?
        .split_once(": ")
        .context("dep-info dependency rule")?;
    // rustc's first rule names its own .d, not an arbitrary caller rule.
    same(Path::new(&target.replace("\\ ", " ")), &dep.path)?;
    let mut token = String::new();
    let mut chars = rest.chars().peekable();
    let mut names = Vec::new();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&' ') {
            token.push(' ');
            chars.next();
        } else if c == ' ' {
            ensure!(!token.is_empty(), "dep-info empty dependency");
            names.push(std::mem::take(&mut token));
        } else {
            token.push(c);
        }
    }
    ensure!(!token.is_empty(), "dep-info final dependency");
    names.push(token);
    ensure!(names.len() <= 4096, "dep-info dependency bound");
    names
        .into_iter()
        .map(|s| {
            // Compiler include! concat paths can contain lexical ../ segments;
            // resolve them before applying the ordinary nonreparse path policy.
            let mut normalized = PathBuf::new();
            for c in root.join(s).components() {
                match c {
                    std::path::Component::ParentDir => {
                        ensure!(normalized.pop(), "dep-info lexical parent");
                    }
                    std::path::Component::CurDir => {}
                    c => normalized.push(c.as_os_str()),
                }
            }
            let p = path(&normalized, false)?;
            ensure!(
                p.starts_with(path(root, false)?),
                "workspace dep-info containment"
            );
            Ok(p)
        })
        .collect()
}
