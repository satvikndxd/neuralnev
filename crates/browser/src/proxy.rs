//! Proxy management: config, health checks, latency-based selection.
//!
//! Health checks are pluggable (`ProxyProber`) so the selection logic is
//! testable without network access; the default prober is a stub that treats
//! configured proxies as healthy with a nominal latency. Real probing would
//! issue a HEAD request through each proxy.

use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProxyHealth {
    pub proxy: ProxyConfig,
    pub healthy: bool,
    pub latency: Duration,
}

pub trait ProxyProber: Send + Sync {
    fn probe(&self, proxy: &ProxyConfig) -> ProxyHealth;
}

/// Deterministic stub prober: latency derived from the proxy name so tests
/// and demos are stable.
pub struct StubProber;

impl ProxyProber for StubProber {
    fn probe(&self, proxy: &ProxyConfig) -> ProxyHealth {
        let pseudo = proxy.name.bytes().map(u64::from).sum::<u64>() % 180 + 40;
        ProxyHealth { proxy: proxy.clone(), healthy: true, latency: Duration::from_millis(pseudo) }
    }
}

pub struct ProxyManager {
    proxies: Vec<ProxyConfig>,
    prober: Box<dyn ProxyProber>,
}

impl ProxyManager {
    pub fn new(proxies: Vec<ProxyConfig>, prober: Box<dyn ProxyProber>) -> Self {
        Self { proxies, prober }
    }

    /// Parse `DEFAULT_PROXY` env format: `name1=url1,name2=url2` or bare url.
    pub fn from_env_value(raw: &str) -> Vec<ProxyConfig> {
        raw.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .enumerate()
            .map(|(i, entry)| match entry.split_once('=') {
                Some((name, url)) => ProxyConfig { name: name.into(), url: url.into() },
                None => ProxyConfig { name: format!("proxy-{}", i + 1), url: entry.into() },
            })
            .collect()
    }

    /// Probe all proxies and pick the healthy one with the lowest latency.
    pub fn select_fastest(&self) -> Option<ProxyHealth> {
        self.proxies
            .iter()
            .map(|p| self.prober.probe(p))
            .filter(|h| h.healthy)
            .min_by_key(|h| h.latency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedProber;
    impl ProxyProber for FixedProber {
        fn probe(&self, proxy: &ProxyConfig) -> ProxyHealth {
            let (healthy, ms) = match proxy.name.as_str() {
                "fast" => (true, 30),
                "slow" => (true, 220),
                _ => (false, 999),
            };
            ProxyHealth { proxy: proxy.clone(), healthy, latency: Duration::from_millis(ms) }
        }
    }

    #[test]
    fn env_parsing_supports_named_and_bare() {
        let ps = ProxyManager::from_env_value("edge=http://p1:8080, http://p2:8080");
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[0].name, "edge");
        assert_eq!(ps[1].name, "proxy-2");
    }

    #[test]
    fn fastest_healthy_proxy_wins() {
        let mgr = ProxyManager::new(
            ProxyManager::from_env_value("dead=http://x,slow=http://s,fast=http://f"),
            Box::new(FixedProber),
        );
        let pick = mgr.select_fastest().unwrap();
        assert_eq!(pick.proxy.name, "fast");
    }
}
