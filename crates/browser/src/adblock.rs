//! Request-interception filter engine.
//!
//! In real mode the Playwright worker consults this list (passed at startup)
//! to abort ad/tracker requests. The embedded default list is a compact seed
//! of well-known ad/tracker domains; `AdblockEngine::from_rules_file` can
//! load a full EasyList-style domain dump (65k+ rules) if provided.

use std::collections::HashSet;

const DEFAULT_RULES: &str = include_str!("adblock_default_rules.txt");

pub struct AdblockEngine {
    domains: HashSet<String>,
    enabled: bool,
}

impl AdblockEngine {
    pub fn with_default_rules(enabled: bool) -> Self {
        Self { domains: parse_rules(DEFAULT_RULES), enabled }
    }

    pub fn from_rules(rules: &str, enabled: bool) -> Self {
        Self { domains: parse_rules(rules), enabled }
    }

    pub fn rule_count(&self) -> usize {
        self.domains.len()
    }

    /// Should this request URL be blocked?
    pub fn is_blocked(&self, url: &str) -> bool {
        if !self.enabled {
            return false;
        }
        let host = match host_of(url) {
            Some(h) => h,
            None => return false,
        };
        // Match the host or any parent domain against the rule set.
        let mut candidate = host.as_str();
        loop {
            if self.domains.contains(candidate) {
                return true;
            }
            match candidate.find('.') {
                Some(i) if i + 1 < candidate.len() => candidate = &candidate[i + 1..],
                _ => return false,
            }
        }
    }

    /// Domains as a plain list (handed to the Playwright worker at spawn).
    pub fn domains(&self) -> Vec<String> {
        self.domains.iter().cloned().collect()
    }
}

fn parse_rules(raw: &str) -> HashSet<String> {
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .map(|l| l.trim_start_matches("||").trim_end_matches('^').to_lowercase())
        .collect()
}

fn host_of(url: &str) -> Option<String> {
    let rest = url.split("//").nth(1).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.split('@').last()?.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_known_tracker_and_subdomains() {
        let ab = AdblockEngine::with_default_rules(true);
        assert!(ab.rule_count() > 20);
        assert!(ab.is_blocked("https://doubleclick.net/ads"));
        assert!(ab.is_blocked("https://stats.g.doubleclick.net/pixel?x=1"));
        assert!(!ab.is_blocked("https://www.amazon.in/s?k=keyboard"));
    }

    #[test]
    fn disabled_engine_blocks_nothing() {
        let ab = AdblockEngine::with_default_rules(false);
        assert!(!ab.is_blocked("https://doubleclick.net/ads"));
    }

    #[test]
    fn easylist_style_lines_parse() {
        let ab = AdblockEngine::from_rules("||adservice.example^\n! comment\n# also comment\n", true);
        assert_eq!(ab.rule_count(), 1);
        assert!(ab.is_blocked("http://adservice.example/x"));
    }
}
