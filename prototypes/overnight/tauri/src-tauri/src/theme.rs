//! Read Omarchy's palette without granting the webview filesystem access.
use std::{collections::BTreeMap, env, fs::File, io::Read, path::PathBuf};

pub fn path() -> Option<PathBuf> {
    if let Some(path) = env::var_os("READER_THEME_FILE").filter(|p| !p.is_empty()) {
        return Some(path.into());
    }
    if cfg!(target_os = "linux") {
        env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".local/state/omarchy/current/theme/colors.toml"))
    } else {
        None
    }
}

pub fn read() -> BTreeMap<String, String> {
    let Some(path) = path() else {
        return BTreeMap::new();
    };
    let Ok(file) = File::open(path) else {
        return BTreeMap::new();
    };
    let mut text = String::new();
    if file.take(65_537).read_to_string(&mut text).is_err() || text.len() > 65_536 {
        return BTreeMap::new();
    }
    parse(&text)
}

fn parse(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.trim().split_once('=')?;
            let key = key.trim();
            let value = value.trim();
            let quote = value.chars().next()?;
            if !matches!(quote, '\'' | '"') {
                return None;
            }
            let (value, _) = value.get(1..)?.split_once(quote)?;
            let valid = if key == "mode" {
                matches!(value, "dark" | "light")
            } else {
                matches!(
                    key,
                    "background"
                        | "foreground"
                        | "accent"
                        | "selection"
                        | "selection_background"
                        | "selection_foreground"
                        | "bright_foreground"
                        | "muted"
                ) && value.len() == 7
                    && value.starts_with('#')
                    && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
            };
            valid.then(|| (key.to_owned(), value.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse;
    #[test]
    fn accepts_omarchy_colors_and_ignores_untrusted_values() {
        let colors = parse(
            "background = \"#1a1b26\" # comment\nforeground = '#a9b1d6'\nmode = \"dark\"\naccent = \"url(https://invalid)\"\n# muted = \"#ffffff\"",
        );
        assert_eq!(colors.len(), 3);
        assert_eq!(
            colors.get("background").map(String::as_str),
            Some("#1a1b26")
        );
        assert_eq!(colors.get("mode").map(String::as_str), Some("dark"));
    }
}
