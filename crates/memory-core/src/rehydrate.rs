//! Rehydratable context compression.
//!
//! Instead of sending whole files into a prompt, the compiler can send a
//! compact fact plus a *handle* the agent can expand on demand. A handle looks
//! like `mem://file/src/auth.rs#L10-L50` or `mem://memory/<id>`. This keeps the
//! upfront token load tiny while preserving the ability to fetch exact code.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// A parsed rehydration handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Handle {
    File {
        path: String,
        start: Option<u64>,
        end: Option<u64>,
    },
    Memory(String),
    Symbol(String),
    Test(String),
    Adr(String),
}

impl Handle {
    /// Parse a `mem://...` handle.
    pub fn parse(s: &str) -> Option<Handle> {
        let rest = s.strip_prefix("mem://")?;
        if let Some(path) = rest.strip_prefix("file/") {
            // optional #L10-L50 or #L10
            if let Some((file, span)) = path.split_once('#') {
                let span = span.trim_start_matches('L');
                let (start, end) = match span.split_once("-L") {
                    Some((a, b)) => (a.parse().ok(), b.parse().ok()),
                    None => {
                        let single = span.parse().ok();
                        (single, single)
                    }
                };
                return Some(Handle::File {
                    path: file.to_string(),
                    start,
                    end,
                });
            }
            return Some(Handle::File {
                path: path.to_string(),
                start: None,
                end: None,
            });
        }
        if let Some(id) = rest.strip_prefix("memory/") {
            return Some(Handle::Memory(id.to_string()));
        }
        if let Some(name) = rest.strip_prefix("symbol/") {
            return Some(Handle::Symbol(name.to_string()));
        }
        if let Some(name) = rest.strip_prefix("test/") {
            return Some(Handle::Test(name.to_string()));
        }
        if let Some(id) = rest.strip_prefix("adr/") {
            return Some(Handle::Adr(id.to_string()));
        }
        None
    }

    /// Render back to the `mem://` form.
    pub fn to_uri(&self) -> String {
        match self {
            Handle::File { path, start, end } => match (start, end) {
                (Some(a), Some(b)) if a == b => format!("mem://file/{path}#L{a}"),
                (Some(a), Some(b)) => format!("mem://file/{path}#L{a}-L{b}"),
                _ => format!("mem://file/{path}"),
            },
            Handle::Memory(id) => format!("mem://memory/{id}"),
            Handle::Symbol(s) => format!("mem://symbol/{s}"),
            Handle::Test(t) => format!("mem://test/{t}"),
            Handle::Adr(a) => format!("mem://adr/{a}"),
        }
    }
}

/// Result of rehydrating a file handle from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rehydrated {
    pub handle: String,
    pub found: bool,
    pub content: String,
}

/// Rehydrate a file handle by reading the requested line span from `root`.
pub fn rehydrate_file(root: &Path, handle: &Handle) -> Rehydrated {
    if let Handle::File { path, start, end } = handle {
        let full = root.join(path);
        match std::fs::read_to_string(&full) {
            Ok(text) => {
                let lines: Vec<&str> = text.lines().collect();
                let (s, e) = match (start, end) {
                    (Some(a), Some(b)) => (*a as usize, *b as usize),
                    _ => (1, lines.len()),
                };
                let s = s.max(1);
                let e = e.min(lines.len()).max(s);
                let content = lines
                    .get(s - 1..e)
                    .map(|slice| slice.join("\n"))
                    .unwrap_or_default();
                Rehydrated {
                    handle: handle.to_uri(),
                    found: !content.is_empty(),
                    content,
                }
            }
            Err(_) => Rehydrated {
                handle: handle.to_uri(),
                found: false,
                content: String::new(),
            },
        }
    } else {
        Rehydrated {
            handle: handle.to_uri(),
            found: false,
            content: "only file handles can be rehydrated from disk".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_and_roundtrips_file_span() {
        let h = Handle::parse("mem://file/src/auth.rs#L10-L50").unwrap();
        assert_eq!(
            h,
            Handle::File {
                path: "src/auth.rs".into(),
                start: Some(10),
                end: Some(50)
            }
        );
        assert_eq!(h.to_uri(), "mem://file/src/auth.rs#L10-L50");
    }

    #[test]
    fn parses_single_line_and_other_kinds() {
        assert_eq!(
            Handle::parse("mem://file/a.rs#L7"),
            Some(Handle::File {
                path: "a.rs".into(),
                start: Some(7),
                end: Some(7)
            })
        );
        assert_eq!(
            Handle::parse("mem://memory/abc"),
            Some(Handle::Memory("abc".into()))
        );
        assert_eq!(Handle::parse("notahandle"), None);
    }

    #[test]
    fn rehydrates_span_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("src.rs");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "l1\nl2\nl3\nl4\nl5").unwrap();
        let h = Handle::File {
            path: "src.rs".into(),
            start: Some(2),
            end: Some(4),
        };
        let r = rehydrate_file(dir.path(), &h);
        assert!(r.found);
        assert_eq!(r.content, "l2\nl3\nl4");
    }
}
