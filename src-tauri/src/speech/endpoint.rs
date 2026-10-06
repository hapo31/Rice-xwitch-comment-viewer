use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct BouyomiAddress {
    host: String,
    port: u16,
}

impl BouyomiAddress {
    pub(crate) fn host(&self) -> &str {
        &self.host
    }
    pub(crate) fn port(&self) -> u16 {
        self.port
    }
    pub fn new(host: impl AsRef<str>, port: u16) -> Result<Self, String> {
        if port == 0 {
            return Err("棒読みちゃんのポート番号が無効です。".to_string());
        }

        Ok(Self {
            host: validate_bouyomi_host(host.as_ref())?,
            port,
        })
    }

    pub(crate) fn display(&self) -> String {
        if matches!(self.host.parse::<IpAddr>(), Ok(IpAddr::V6(_))) {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

pub fn validate_bouyomi_host(host: &str) -> Result<String, String> {
    let raw_bytes = host.len();
    let raw_controls = host.chars().any(char::is_control);
    let host = host.trim();
    let invalid = || {
        "棒読みちゃんのホストが無効です。IPv4、DNS名、または角括弧なしのIPv6アドレスを入力してください。"
            .to_string()
    };

    if host.is_empty()
        || raw_bytes > 253
        || raw_controls
        || host.contains(char::is_whitespace)
        || host.contains(['[', ']'])
    {
        return Err(invalid());
    }

    if host.contains(':') {
        return host
            .parse::<IpAddr>()
            .ok()
            .filter(|address| address.is_ipv6())
            .map(|_| host.to_string())
            .ok_or_else(invalid);
    }

    if host.parse::<IpAddr>().is_ok()
        || host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
    {
        Ok(host.to_string())
    } else {
        Err(invalid())
    }
}
