use super::outcome::BlockedReason;
use crate::settings::UrlHandling;
use crate::twitch::{ChatMessage, MessageFragment};
use linkify::{LinkFinder, LinkKind};
use std::ops::Range;

pub(super) const DEFAULT_MAX_COMMENT_LENGTH: usize = 120;

#[derive(Debug, Clone)]
pub struct SpeechFormatter {
    options: SpeechFormatterOptions,
}

#[derive(Debug, Clone)]
pub struct SpeechFormatterOptions {
    pub read_user_name: bool,
    pub max_comment_length: usize,
    pub replace_urls: bool,
    pub block_urls: bool,
    pub escape_bouyomi_tags: bool,
    pub read_emotes: bool,
    pub blocked_users: Vec<String>,
    pub blocked_words: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeechFormatDecision {
    Speak(String),
    Blocked(BlockedReason),
}

impl Default for SpeechFormatterOptions {
    fn default() -> Self {
        Self {
            read_user_name: true,
            max_comment_length: DEFAULT_MAX_COMMENT_LENGTH,
            replace_urls: true,
            block_urls: false,
            escape_bouyomi_tags: true,
            read_emotes: false,
            blocked_users: Vec::new(),
            blocked_words: Vec::new(),
        }
    }
}

impl SpeechFormatter {
    pub fn new(options: SpeechFormatterOptions) -> Self {
        Self { options }
    }

    pub fn format_chat_message(&self, message: &ChatMessage) -> SpeechFormatDecision {
        let raw_text = collect_readable_text(message, self.options.read_emotes);
        if raw_text.trim().is_empty() {
            return SpeechFormatDecision::Blocked(BlockedReason::EmptyAfterFormatting);
        }

        if contains_blocked_user(&self.options.blocked_users, message) {
            return SpeechFormatDecision::Blocked(BlockedReason::BlockedUser);
        }

        if self.options.block_urls && contains_url(&raw_text) {
            return SpeechFormatDecision::Blocked(BlockedReason::BlockedUrl);
        }

        let lowered = raw_text.to_ascii_lowercase();
        if self
            .options
            .blocked_words
            .iter()
            .map(|word| word.trim())
            .filter(|word| !word.is_empty())
            .any(|word| lowered.contains(&word.to_ascii_lowercase()))
        {
            return SpeechFormatDecision::Blocked(BlockedReason::BlockedWord);
        }

        let mut text = normalize_control_chars(&raw_text);
        if self.options.replace_urls {
            text = replace_urls(&text);
        }
        if self.options.escape_bouyomi_tags {
            text = escape_bouyomi_tags(&text);
        }
        text = collapse_spaces(&text);

        // Normalization and omitted emotes can remove every readable character. Do not
        // turn that into an empty packet or a display-name-only utterance.
        if text.is_empty() {
            return SpeechFormatDecision::Blocked(BlockedReason::EmptyAfterFormatting);
        }

        if self.options.read_user_name {
            text = format!("{}。{}", message.user_display_name, text);
        }

        let max_len = self.options.max_comment_length.max(1);
        if text.chars().count() > max_len {
            text = truncate_chars(&text, max_len);
        }

        SpeechFormatDecision::Speak(text)
    }
}

impl From<&crate::settings::SpeechSettings> for SpeechFormatterOptions {
    fn from(settings: &crate::settings::SpeechSettings) -> Self {
        Self {
            read_user_name: settings.read_user_name,
            max_comment_length: settings.max_comment_length as usize,
            replace_urls: matches!(settings.url_handling, UrlHandling::Replace),
            block_urls: matches!(settings.url_handling, UrlHandling::Block),
            escape_bouyomi_tags: true,
            read_emotes: settings.read_emotes,
            blocked_users: settings.blocked_users.clone(),
            blocked_words: settings.blocked_words.clone(),
        }
    }
}

fn collect_readable_text(message: &ChatMessage, read_emotes: bool) -> String {
    if message.fragments.is_empty() {
        return message.text.clone();
    }

    message
        .fragments
        .iter()
        .filter_map(|fragment| readable_fragment_text(fragment, read_emotes))
        .collect::<Vec<_>>()
        .join("")
}

fn readable_fragment_text(fragment: &MessageFragment, read_emotes: bool) -> Option<String> {
    if fragment.emote.is_some() && !read_emotes {
        return None;
    }
    Some(fragment.text.clone())
}

fn normalize_control_chars(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_control() || ch == '\n' || ch == '\r' || ch == '\t' {
                ' '
            } else {
                ch
            }
        })
        .collect()
}

fn replace_urls(text: &str) -> String {
    let ranges = find_url_ranges(text);
    if ranges.is_empty() {
        return text.to_string();
    }

    let mut replaced = String::with_capacity(text.len());
    let mut cursor = 0;
    for range in ranges {
        replaced.push_str(&text[cursor..range.start]);
        replaced.push_str("URL省略");
        cursor = range.end;
    }
    replaced.push_str(&text[cursor..]);
    replaced
}

pub(super) fn contains_url(text: &str) -> bool {
    !find_url_ranges(text).is_empty()
}

/// Uses linkify to locate candidate URL spans, then applies Rice's narrower
/// accepted formats and URL parser validation before returning any byte range.
/// The compatibility parser scans the full input when linkify produced no
/// accepted candidate and checks bracketed IPv6 even when other candidates exist.
pub(super) fn find_url_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut finder = LinkFinder::new();
    finder
        .kinds(&[LinkKind::Url])
        .url_must_have_scheme(false)
        .url_can_be_iri(false);

    for link in finder.links(text) {
        let mut index = link.start();
        while index < link.end() {
            if let Some(range) = url_range_at(text, index) {
                if range.end <= link.end() {
                    index = range.end;
                    ranges.push(range);
                    continue;
                }
            }
            index += text[index..]
                .chars()
                .next()
                .expect("index is within UTF-8 text")
                .len_utf8();
        }
    }

    // linkify intentionally keeps its URL grammar broad and currently skips
    // some supported forms, such as bracketed IPv6 authorities and URLs
    // followed immediately by ASCII prose delimiters. If it produced no
    // accepted candidate, preserve those cases with the compatibility parser.
    if ranges.is_empty() {
        let mut index = 0;
        while index < text.len() {
            if let Some(range) = url_range_at(text, index) {
                index = range.end;
                ranges.push(range);
                continue;
            }
            index += text[index..]
                .chars()
                .next()
                .expect("index is within UTF-8 text")
                .len_utf8();
        }
        return ranges;
    }

    // Bracketed IPv6 is the only known supported form that can be missed when
    // another candidate caused the fallback scan above to be skipped.
    let mut index = 0;
    while index < text.len() {
        if is_bracketed_ip_literal_start(text, index) {
            if let Some(range) = url_range_at(text, index) {
                if !ranges.iter().any(|existing| existing == &range) {
                    ranges.push(range.clone());
                }
                index = range.end;
                continue;
            }
        }
        index += text[index..]
            .chars()
            .next()
            .expect("index is within UTF-8 text")
            .len_utf8();
    }

    ranges.sort_by_key(|range| range.start);
    ranges
}

fn is_bracketed_ip_literal_start(text: &str, start: usize) -> bool {
    if !is_url_start_boundary(text, start) {
        return false;
    }
    let remaining = &text.as_bytes()[start..];
    starts_with_ascii_case_insensitive(remaining, b"https://[")
        || starts_with_ascii_case_insensitive(remaining, b"http://[")
}

fn url_range_at(text: &str, start: usize) -> Option<Range<usize>> {
    if !is_url_start_boundary(text, start) {
        return None;
    }

    let remaining = &text.as_bytes()[start..];
    let (prefix_len, scheme_less) = if starts_with_ascii_case_insensitive(remaining, b"https://") {
        (b"https://".len(), false)
    } else if starts_with_ascii_case_insensitive(remaining, b"http://") {
        (b"http://".len(), false)
    } else if starts_with_ascii_case_insensitive(remaining, b"www.") {
        (b"www.".len(), true)
    } else {
        return None;
    };

    let mut end = start + prefix_len;
    while end < text.len() && is_url_ascii_byte(text.as_bytes()[end]) {
        end += 1;
    }

    // ASCII opening delimiters can immediately follow a URL before prose. Try
    // the complete candidate first so balanced path delimiters remain part of
    // the URL, then backtrack only to delimiter boundaries. Backtracking every
    // byte could turn an invalid authority such as port 65536 into a different,
    // apparently valid URL by dropping its final digit.
    end = trim_url_suffix(text, start + prefix_len, end);
    if end > start + prefix_len && is_valid_url_candidate(&text[start..end], scheme_less) {
        return Some(start..end);
    }

    while let Some(delimiter_offset) = text[start + prefix_len..end]
        .bytes()
        .rposition(is_ascii_opening_delimiter)
    {
        end = start + prefix_len + delimiter_offset;
        end = trim_url_suffix(text, start + prefix_len, end);
        if end > start + prefix_len && is_valid_url_candidate(&text[start..end], scheme_less) {
            return Some(start..end);
        }
    }

    None
}

fn starts_with_ascii_case_insensitive(value: &[u8], prefix: &[u8]) -> bool {
    value.len() >= prefix.len() && value[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn is_url_start_boundary(text: &str, start: usize) -> bool {
    let Some(previous) = text[..start].chars().next_back() else {
        return true;
    };

    !previous.is_ascii_alphanumeric() && !matches!(previous, '@' | '.' | '_' | '-')
}

fn is_valid_url_candidate(candidate: &str, scheme_less: bool) -> bool {
    let parsed = if scheme_less {
        reqwest::Url::parse(&format!("http://{candidate}"))
    } else {
        reqwest::Url::parse(candidate)
    };

    parsed.is_ok_and(|url| url.host_str().is_some_and(is_valid_url_host))
}

fn is_valid_url_host(host: &str) -> bool {
    let unbracketed = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    if unbracketed.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }

    unbracketed.split('.').all(|label| {
        !label.is_empty()
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && label
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && label
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
    })
}

fn is_url_ascii_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b':'
                | b'/'
                | b'?'
                | b'#'
                | b'['
                | b']'
                | b'@'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b'%'
        )
}

fn is_ascii_opening_delimiter(byte: u8) -> bool {
    matches!(byte, b'(' | b'[' | b'\'')
}

fn trim_url_suffix(text: &str, content_start: usize, mut end: usize) -> usize {
    let mut trimmed_closing_ascii_single_quote = false;

    while end > content_start {
        let byte = text.as_bytes()[end - 1];
        if byte == b'\'' && !trimmed_closing_ascii_single_quote {
            end -= 1;
            trimmed_closing_ascii_single_quote = true;
            continue;
        }

        if matches!(byte, b'.' | b',' | b'!' | b'?' | b';' | b':') {
            end -= 1;
            continue;
        }

        if byte == b')'
            && text[content_start..end]
                .bytes()
                .filter(|byte| *byte == b'(')
                .count()
                < text[content_start..end]
                    .bytes()
                    .filter(|byte| *byte == b')')
                    .count()
        {
            end -= 1;
            continue;
        }

        if byte == b']'
            && text[content_start..end]
                .bytes()
                .filter(|byte| *byte == b'[')
                .count()
                < text[content_start..end]
                    .bytes()
                    .filter(|byte| *byte == b']')
                    .count()
        {
            end -= 1;
            continue;
        }

        if byte == b'('
            && text[content_start..end]
                .bytes()
                .filter(|byte| *byte == b'(')
                .count()
                > text[content_start..end]
                    .bytes()
                    .filter(|byte| *byte == b')')
                    .count()
        {
            end -= 1;
            continue;
        }

        if byte == b'['
            && text[content_start..end]
                .bytes()
                .filter(|byte| *byte == b'[')
                .count()
                > text[content_start..end]
                    .bytes()
                    .filter(|byte| *byte == b']')
                    .count()
        {
            end -= 1;
            continue;
        }

        break;
    }
    end
}

pub(super) fn contains_blocked_user(blocked_users: &[String], message: &ChatMessage) -> bool {
    blocked_users.iter().any(|user| {
        let user = user.trim().trim_start_matches('@');
        !user.is_empty()
            && (message.user_login.eq_ignore_ascii_case(user)
                || message.user_display_name.eq_ignore_ascii_case(user))
    })
}

fn escape_bouyomi_tags(text: &str) -> String {
    text.replace(')', "）").replace('(', "（")
}

fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }

    let mut output = text
        .chars()
        .take(max_len.saturating_sub(1))
        .collect::<String>();
    output.push('…');
    output
}
