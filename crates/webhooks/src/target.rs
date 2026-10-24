use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use url::{Host, Url};

/// IPv6 address of the AWS instance metadata service (Nitro instances).
const AWS_METADATA_V6: Ipv6Addr = Ipv6Addr::new(0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254);

/// Parses a webhook callback URL and ensures it is an absolute `http` or
/// `https` URL with a host.
pub(crate) fn validate_callback_url(callback_url: &str) -> Result<Url, String> {
    let url = Url::parse(callback_url).map_err(|e| format!("Invalid callback URL: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Callback URL must use http or https".to_string());
    }
    if url.host().is_none() {
        return Err("Callback URL must contain a host".to_string());
    }
    Ok(url)
}

/// Returns `true` if webhook deliveries to `ip` must be refused.
///
/// Link-local and cloud metadata addresses are always blocked. Loopback,
/// private and other non-public addresses are only blocked if
/// `block_private_targets` is set, as webhooks to internal services are a
/// legitimate use case for a self-hosted registry.
pub(crate) fn is_blocked_ip(ip: IpAddr, block_private_targets: bool) -> bool {
    let ip = canonical_ip(ip);
    is_link_local_or_metadata(ip) || (block_private_targets && is_non_public(ip))
}

/// Checks the host of a callback URL if it is an IP literal. Domain names
/// are checked after DNS resolution by [`GuardedResolver`].
pub(crate) fn is_blocked_url(url: &Url, block_private_targets: bool) -> bool {
    match url.host() {
        Some(Host::Ipv4(ip)) => is_blocked_ip(IpAddr::V4(ip), block_private_targets),
        Some(Host::Ipv6(ip)) => is_blocked_ip(IpAddr::V6(ip), block_private_targets),
        Some(Host::Domain(_)) | None => false,
    }
}

/// Maps IPv4-mapped and NAT64 (`64:ff9b::/96`) IPv6 addresses to the
/// embedded IPv4 address, so they cannot be used to bypass IPv4 rules.
fn canonical_ip(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(v6) if v6.segments()[..6] == [0x64, 0xff9b, 0, 0, 0, 0] => {
            let [.., a, b, c, d] = v6.octets();
            IpAddr::V4(Ipv4Addr::new(a, b, c, d))
        }
        other => other,
    }
}

fn is_link_local_or_metadata(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_unicast_link_local() || v6 == AWS_METADATA_V6,
    }
}

fn is_non_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_unspecified()
                || v4.is_broadcast()
                // Shared address space (carrier-grade NAT), 100.64.0.0/10
                || (a == 100 && (b & 0xc0) == 64)
                // "This network", 0.0.0.0/8
                || a == 0
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified() || v6.is_unique_local(),
    }
}

/// DNS resolver that drops resolved addresses webhooks must not be
/// delivered to. Checking after resolution covers host names that point to
/// blocked addresses, and because the checked addresses are the ones used to
/// connect, DNS rebinding cannot bypass the check.
pub(crate) struct GuardedResolver {
    pub(crate) block_private_targets: bool,
}

impl Resolve for GuardedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let block_private_targets = self.block_private_targets;
        Box::pin(async move {
            let host = name.as_str();
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, 0))
                .await?
                .filter(|addr| !is_blocked_ip(addr.ip(), block_private_targets))
                .collect();
            if addrs.is_empty() {
                return Err(format!("Webhook target {host} has no permitted address").into());
            }
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn always_blocks_link_local_and_metadata() {
        for addr in [
            "169.254.169.254",
            "169.254.0.1",
            "fe80::1",
            "febf::1",
            "fd00:ec2::254",
            "::ffff:169.254.169.254",
            "64:ff9b::a9fe:a9fe",
        ] {
            assert!(is_blocked_ip(ip(addr), false), "{addr}");
            assert!(is_blocked_ip(ip(addr), true), "{addr}");
        }
    }

    #[test]
    fn allows_private_targets_by_default() {
        for addr in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "0.0.0.0",
            "::1",
            "fd00::1",
            "8.8.8.8",
            "2001:4860:4860::8888",
        ] {
            assert!(!is_blocked_ip(ip(addr), false), "{addr}");
        }
    }

    #[test]
    fn blocks_private_targets_if_configured() {
        for addr in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "100.64.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "::1",
            "::",
            "fd00::1",
            "::ffff:10.0.0.1",
            "64:ff9b::7f00:1",
        ] {
            assert!(is_blocked_ip(ip(addr), true), "{addr}");
        }
    }

    #[test]
    fn allows_public_targets_if_private_targets_are_blocked() {
        for addr in ["8.8.8.8", "100.128.0.1", "2001:4860:4860::8888"] {
            assert!(!is_blocked_ip(ip(addr), true), "{addr}");
        }
    }

    #[test]
    fn checks_ip_literal_hosts_in_urls() {
        let blocked = Url::parse("http://169.254.169.254/latest/meta-data").unwrap();
        let blocked_v6 = Url::parse("http://[fd00:ec2::254]/").unwrap();
        let private = Url::parse("http://10.0.0.1/hook").unwrap();
        let domain = Url::parse("http://metadata.example/").unwrap();

        assert!(is_blocked_url(&blocked, false));
        assert!(is_blocked_url(&blocked_v6, false));
        assert!(!is_blocked_url(&private, false));
        assert!(is_blocked_url(&private, true));
        assert!(!is_blocked_url(&domain, true));
    }

    #[tokio::test]
    async fn resolver_rejects_hosts_resolving_to_blocked_addresses() {
        let resolver = GuardedResolver {
            block_private_targets: true,
        };
        let result = resolver.resolve("localhost".parse().unwrap()).await;
        assert!(result.is_err());

        let resolver = GuardedResolver {
            block_private_targets: false,
        };
        let addrs: Vec<SocketAddr> = resolver
            .resolve("localhost".parse().unwrap())
            .await
            .unwrap()
            .collect();
        assert!(!addrs.is_empty());
    }

    #[test]
    fn accepts_http_and_https_urls() {
        assert!(validate_callback_url("http://my-service:8000").is_ok());
        assert!(validate_callback_url("https://example.com/hook?x=1").is_ok());
        assert!(validate_callback_url("http://10.0.0.1/hook").is_ok());
    }

    #[test]
    fn rejects_invalid_urls() {
        assert!(validate_callback_url("").is_err());
        assert!(validate_callback_url("not a url").is_err());
        assert!(validate_callback_url("/relative/path").is_err());
    }

    #[test]
    fn rejects_non_http_schemes() {
        assert!(validate_callback_url("file:///etc/passwd").is_err());
        assert!(validate_callback_url("ftp://example.com").is_err());
        assert!(validate_callback_url("unix:/var/run/docker.sock").is_err());
    }
}
