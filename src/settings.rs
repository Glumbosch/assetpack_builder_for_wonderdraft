use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub fn default_wonderdraft_folder() -> PathBuf {
    let home = PathBuf::from(env::var_os("HOME").unwrap_or_else(|| ".".into()));
    if cfg!(target_os = "windows") {
        PathBuf::from(
            env::var_os("APPDATA").unwrap_or_else(|| home.join("AppData/Roaming").into_os_string()),
        )
        .join("Wonderdraft")
    } else if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Wonderdraft")
    } else {
        home.join(".local/share/Wonderdraft")
    }
}

pub fn find_install_root() -> Option<PathBuf> {
    install_root_from_wonderdraft_folder(&default_wonderdraft_folder())
}

pub fn install_root_from_selection(selection: &Path) -> Option<PathBuf> {
    if selection.join("config.ini").is_file() {
        return install_root_from_wonderdraft_folder(selection);
    }

    if selection.file_name().is_some_and(|name| name == "assets") {
        return selection.parent().map(Path::to_owned);
    }

    Some(selection.to_owned())
}

fn install_root_from_wonderdraft_folder(folder: &Path) -> Option<PathBuf> {
    let config_path = folder.join("config.ini");
    let text = fs::read_to_string(config_path).ok()?;
    let configured = parse_custom_assets_directory(&text);
    let root = match configured {
        Some(path) if path.is_absolute() => path,
        Some(path) => folder.join(path),
        None => folder.to_owned(),
    };

    if root.file_name().is_some_and(|name| name == "assets") {
        root.parent().map(Path::to_owned)
    } else {
        Some(root)
    }
}

fn parse_custom_assets_directory(text: &str) -> Option<PathBuf> {
    for raw_line in text.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };
        if raw_key.trim() == "custom_assets_directory" {
            return parse_quoted_value(raw_value.trim()).map(PathBuf::from);
        }
    }
    None
}

fn parse_quoted_value(value: &str) -> Option<String> {
    let mut current = String::new();
    let mut chars = value.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        if !quoted {
            if ch == '"' {
                quoted = true;
            }
            continue;
        }
        match ch {
            '"' => return Some(current),
            '\\' if matches!(chars.peek(), Some('"' | '\\')) => {
                current.push(chars.next().expect("peeked character must exist"));
            }
            _ => current.push(ch),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_custom_asset_override() {
        let config = r#"
[Save]
last_directory="/maps"
custom_assets_directory="/home/test/Wonderdraft2"
"#;
        assert_eq!(
            parse_custom_assets_directory(config),
            Some(PathBuf::from("/home/test/Wonderdraft2"))
        );
    }

    #[test]
    fn parses_escaped_path_characters() {
        assert_eq!(
            parse_quoted_value(r#""C:\\Users\\Map \"Maker\"""#),
            Some(r#"C:\Users\Map "Maker""#.to_owned())
        );
    }

    #[test]
    fn selecting_assets_folder_uses_its_parent_as_export_root() {
        assert_eq!(
            install_root_from_selection(Path::new("/tmp/Wonderdraft/assets")),
            Some(PathBuf::from("/tmp/Wonderdraft"))
        );
    }
}
