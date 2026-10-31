//! Declared tool versions: pure path selection and parsing, with reads left to the file service.

use std::path::{Path, PathBuf};

fn tool(language: &str) -> Option<&'static str> {
    match language {
        "Rust" => Some("rust"),
        "Elixir" | "HEEx" => Some("elixir"),
        "JavaScript" | "TypeScript" | "TSX" => Some("nodejs"),
        _ => None,
    }
}

/// Candidate files, nearest directory first, including files that don't exist yet.
pub fn candidates(path: &Path, language: &str) -> Vec<PathBuf> {
    let Some(tool) = tool(language) else {
        return Vec::new();
    };
    path.ancestors()
        .skip(1)
        .flat_map(|dir| {
            [".tool-versions", "mise.toml", "rust-toolchain.toml"]
                .into_iter()
                .filter(move |name| *name != "rust-toolchain.toml" || tool == "rust")
                .map(move |name| dir.join(name))
        })
        .collect()
}

/// The first declared version; `Some("")` stops inheritance without supplying a display value.
pub fn parse(path: &Path, language: &str, text: &str) -> Option<String> {
    let tool = tool(language)?;
    let value = match path.file_name()?.to_str()? {
        ".tool-versions" => {
            let mut entry = text.lines().find_map(|line| {
                let mut words = line.split('#').next()?.split_whitespace();
                (words.next()? == tool).then_some(words)
            })?;
            entry.next().unwrap_or("").to_string()
        }
        "mise.toml" | "rust-toolchain.toml" => {
            let config: toml::Value = toml::from_str(text).ok()?;
            let value = match path.file_name()?.to_str()? {
                "rust-toolchain.toml" if tool == "rust" => {
                    config.get("toolchain")?.get("channel")?
                }
                "mise.toml" => {
                    let tools = config.get("tools")?;
                    tools
                        .get(if tool == "nodejs" { "node" } else { tool })
                        .or_else(|| tools.get(tool))?
                }
                _ => return None,
            };
            fn version(value: &toml::Value) -> &str {
                match value {
                    toml::Value::String(s) => s,
                    toml::Value::Array(a) => a.first().map_or("", version),
                    toml::Value::Table(t) => t.get("version").map_or("", version),
                    _ => "",
                }
            }
            version(value).to_string()
        }
        _ => return None,
    };
    let usable = !value.is_empty()
        && value != "system"
        && !value.contains(['{', '}'])
        && !value.chars().any(|c| c.is_whitespace() || c.is_control());
    Some(if usable { value } else { String::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_format_supplies_its_first_declared_version() {
        for (file, language, text, expected) in [
            (
                ".tool-versions",
                "Rust",
                "# tools\nelixir 1.20.2-otp-26\nrust 1.99.0 1.98.0 # fallback\n",
                "1.99.0",
            ),
            (
                ".tool-versions",
                "Elixir",
                "elixir 1.20.2-otp-26",
                "1.20.2-otp-26",
            ),
            (
                ".tool-versions",
                "HEEx",
                "elixir 1.20.2-otp-26",
                "1.20.2-otp-26",
            ),
            (".tool-versions", "JavaScript", "nodejs 24.0.0", "24.0.0"),
            ("mise.toml", "Rust", "[tools]\nrust = '1.99.0'", "1.99.0"),
            (
                "mise.toml",
                "Elixir",
                "[tools]\nelixir = {version = '1.20.2-otp-26'}",
                "1.20.2-otp-26",
            ),
            (
                "mise.toml",
                "TypeScript",
                "[tools]\nnode = ['24.0.0', '22.0.0']",
                "24.0.0",
            ),
            ("mise.toml", "TSX", "[tools]\nnodejs = '24.0.0'", "24.0.0"),
            (
                "rust-toolchain.toml",
                "Rust",
                "[toolchain]\nchannel = 'nightly-2026-10-01'",
                "nightly-2026-10-01",
            ),
        ] {
            assert_eq!(
                parse(Path::new(file), language, text).as_deref(),
                Some(expected),
                "{file}: {language}"
            );
        }
    }

    #[test]
    fn explicit_unusable_entries_stop_inheritance() {
        for value in [
            "''",
            "'system'",
            "[]",
            "42",
            "{path = '/tmp/rust'}",
            "'{{env.VERSION}}'",
            "'1.99 0'",
            "\"1.99\\n0\"",
        ] {
            let text = format!("[tools]\nrust = {value}");
            assert_eq!(
                parse(Path::new("mise.toml"), "Rust", &text),
                Some(String::new()),
                "{value}"
            );
        }
        assert_eq!(
            parse(Path::new(".tool-versions"), "Rust", "rust # empty"),
            Some(String::new())
        );
    }

    #[test]
    fn absent_tools_and_malformed_files_allow_fallback() {
        for text in ["not = [valid", "[tools]\nnode = '24'"] {
            assert_eq!(parse(Path::new("mise.toml"), "Rust", text), None);
        }
        assert_eq!(
            parse(Path::new(".tool-versions"), "Rust", "elixir 1.20.2"),
            None
        );
    }

    #[test]
    fn candidates_include_missing_pins_up_to_the_root() {
        assert_eq!(
            candidates(Path::new("/a/b.rs"), "Rust"),
            [
                "/a/.tool-versions",
                "/a/mise.toml",
                "/a/rust-toolchain.toml",
                "/.tool-versions",
                "/mise.toml",
                "/rust-toolchain.toml"
            ]
            .map(PathBuf::from)
        );
        assert_eq!(
            candidates(Path::new("/a.ex"), "Elixir"),
            ["/.tool-versions", "/mise.toml"].map(PathBuf::from)
        );
        assert_eq!(
            candidates(Path::new("/a.md"), "Markdown"),
            Vec::<PathBuf>::new()
        );
    }
}
