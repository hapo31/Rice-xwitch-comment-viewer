fn main() {
    println!("cargo:rerun-if-env-changed=RICE_TWITCH_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=TWITCH_CLIENT_ID");
    println!("cargo:rerun-if-changed=../.env");

    let twitch_client_id = env_value("RICE_TWITCH_CLIENT_ID")
        .or_else(|| env_value("TWITCH_CLIENT_ID"))
        .or_else(|| dotenv_value("RICE_TWITCH_CLIENT_ID"))
        .or_else(|| dotenv_value("TWITCH_CLIENT_ID"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if let Some(client_id) = twitch_client_id {
        println!("cargo:rustc-env=RICE_TWITCH_CLIENT_ID={client_id}");
    }

    if std::env::var_os("CARGO_FEATURE_APP").is_some() {
        tauri_build::build();
    }
}

fn env_value(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

fn dotenv_value(key: &str) -> Option<String> {
    let manifest_dir = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR")?);
    let text = std::fs::read_to_string(manifest_dir.join("..").join(".env")).ok()?;

    text.lines()
        .filter_map(parse_dotenv_line)
        .find_map(|(name, value)| (name == key).then_some(value))
}

fn parse_dotenv_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let (name, value) = line.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }

    Some((
        name.to_string(),
        unquote_env_value(value.trim()).to_string(),
    ))
}

fn unquote_env_value(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}
