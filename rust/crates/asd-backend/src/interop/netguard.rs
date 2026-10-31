//! owner: WP-I.7
//!
//! Outbound connection guard (DLV-3, trap 19). An org admin chooses where deliveries go,
//! so a destination is untrusted input: without this guard a webhook URL could reach the
//! cloud metadata service (`169.254.169.254`), the database or any other private service
//! of the deployment (SSRF).
//!
//! The rules:
//!
//! - **Resolve once, check every address, connect to a checked address.** [`resolve`]
//!   returns socket addresses that all passed the check; the HTTP client and the SFTP
//!   client connect to exactly those (`webhook::PinnedResolver`, `sftp::connect`), so a
//!   DNS answer that changes between the check and the connect (rebinding) cannot reach a
//!   private address. A name with *any* refused address is refused as a whole.
//! - **Webhooks** ([`Channel::Http`]) refuse loopback, RFC 1918, link-local (incl. the
//!   metadata address), CGNAT `100.64.0.0/10`, IPv6 loopback/ULA/link-local, multicast,
//!   unspecified, broadcast and documentation/benchmark ranges, plus IPv4-mapped and NAT64
//!   forms of those. `https` only.
//! - **SFTP** ([`Channel::Sftp`]) refuses the same *except* RFC 1918, CGNAT and IPv6 ULA:
//!   hospital drops commonly sit on a private address behind a site VPN, and the pinned
//!   host key (INTG-5) means the connection only completes against the one server the
//!   hospital named. Loopback, link-local/metadata, multicast and unspecified stay refused.
//!
//! Tests that run a receiver on `127.0.0.1` relax the policy process-wide with
//! [`allow_loopback_for_tests`]; production never calls it.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::ErrorClass;

/// DNS lookups give up after this long.
pub const RESOLVE_TIMEOUT: Duration = Duration::from_secs(5);

/// What the connection is for; decides which ranges are refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Http,
    Sftp,
}

static ALLOW_LOOPBACK: AtomicBool = AtomicBool::new(false);

/// Test receivers listen on loopback over plain HTTP: after this call (for the rest of the
/// process) loopback addresses and `http://` URLs are allowed. Every other refused range
/// stays refused. Never called by the service itself.
pub fn allow_loopback_for_tests() {
    ALLOW_LOOPBACK.store(true, Ordering::SeqCst);
}

/// Whether [`allow_loopback_for_tests`] was called in this process.
pub fn loopback_allowed() -> bool {
    ALLOW_LOOPBACK.load(Ordering::SeqCst)
}

fn v4_in(ip: Ipv4Addr, net: [u8; 4], prefix: u32) -> bool {
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    (u32::from(ip) & mask) == (u32::from(Ipv4Addr::from(net)) & mask)
}

fn v6_in(ip: Ipv6Addr, net: Ipv6Addr, prefix: u32) -> bool {
    let mask = if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - prefix)
    };
    (u128::from(ip) & mask) == (u128::from(net) & mask)
}

/// Private-network ranges that SFTP may reach (site VPN) but webhooks may not.
fn v4_private(ip: Ipv4Addr) -> bool {
    v4_in(ip, [10, 0, 0, 0], 8)
        || v4_in(ip, [172, 16, 0, 0], 12)
        || v4_in(ip, [192, 168, 0, 0], 16)
        || v4_in(ip, [100, 64, 0, 0], 10)
}

/// Ranges no channel may reach.
fn v4_never(ip: Ipv4Addr) -> bool {
    v4_in(ip, [0, 0, 0, 0], 8) // "this network", incl. 0.0.0.0
        || v4_in(ip, [127, 0, 0, 0], 8) // loopback
        || v4_in(ip, [169, 254, 0, 0], 16) // link-local, cloud metadata
        || v4_in(ip, [192, 0, 0, 0], 24) // IETF protocol assignments
        || v4_in(ip, [192, 0, 2, 0], 24) // TEST-NET-1
        || v4_in(ip, [198, 18, 0, 0], 15) // benchmarking
        || v4_in(ip, [198, 51, 100, 0], 24) // TEST-NET-2
        || v4_in(ip, [203, 0, 113, 0], 24) // TEST-NET-3
        || v4_in(ip, [224, 0, 0, 0], 4) // multicast
        || v4_in(ip, [240, 0, 0, 0], 4) // reserved, incl. broadcast
}

fn v4_refused(ip: Ipv4Addr, channel: Channel) -> bool {
    if loopback_allowed() && ip.is_loopback() {
        return false;
    }
    v4_never(ip) || (channel == Channel::Http && v4_private(ip))
}

fn v6_refused(ip: Ipv6Addr, channel: Channel) -> bool {
    // IPv4-mapped (`::ffff:a.b.c.d`) and NAT64 (`64:ff9b::a.b.c.d`) carry an IPv4 target.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return v4_refused(v4, channel);
    }
    if v6_in(ip, "64:ff9b::".parse().expect("valid"), 96) {
        let v4 = Ipv4Addr::from(u32::try_from(u128::from(ip) & 0xffff_ffff).expect("32 bits"));
        return v4_refused(v4, channel);
    }
    if loopback_allowed() && ip.is_loopback() {
        return false;
    }
    let never = ip.is_unspecified()
        || ip.is_loopback()
        || v6_in(ip, "fe80::".parse().expect("valid"), 10) // link-local
        || v6_in(ip, "fec0::".parse().expect("valid"), 10) // site-local (deprecated)
        || v6_in(ip, "ff00::".parse().expect("valid"), 8) // multicast
        || v6_in(ip, "2001:db8::".parse().expect("valid"), 32) // documentation
        || v6_in(ip, "::".parse().expect("valid"), 96); // IPv4-compatible (deprecated)
    let private = v6_in(ip, "fc00::".parse().expect("valid"), 7); // ULA
    never || (channel == Channel::Http && private)
}

/// Whether `channel` may not connect to `ip`.
pub fn is_refused(ip: IpAddr, channel: Channel) -> bool {
    match ip {
        IpAddr::V4(v4) => v4_refused(v4, channel),
        IpAddr::V6(v6) => v6_refused(v6, channel),
    }
}

/// Resolves `host:port` once and returns the addresses, all of which passed the check.
/// Any refused address refuses the name (`address-blocked`); no answer is `dns-failed`.
pub async fn resolve(
    host: &str,
    port: u16,
    channel: Channel,
) -> Result<Vec<SocketAddr>, ErrorClass> {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let addrs: Vec<SocketAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::time::timeout(RESOLVE_TIMEOUT, tokio::net::lookup_host((host, port)))
            .await
            .map_err(|_| ErrorClass::DnsFailed)?
            .map_err(|_| ErrorClass::DnsFailed)?
            .collect()
    };
    if addrs.is_empty() {
        return Err(ErrorClass::DnsFailed);
    }
    if addrs.iter().any(|a| is_refused(a.ip(), channel)) {
        return Err(ErrorClass::AddressBlocked);
    }
    Ok(addrs)
}

/// A webhook URL split into what the client needs (DLV-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookUrl {
    pub uri: http::Uri,
    pub host: String,
    pub port: u16,
    pub https: bool,
}

/// Checks the *shape* of a webhook URL: `https` (plain `http` only after
/// [`allow_loopback_for_tests`]), a host, no user info, no fragment, at most 2048 bytes.
/// The `Err` is a reason fit for a validation message (it never repeats the URL).
pub fn parse_webhook_url(raw: &str) -> Result<WebhookUrl, &'static str> {
    if raw.len() > 2048 {
        return Err("must be at most 2048 characters");
    }
    if raw.contains('#') {
        return Err("must not contain a fragment");
    }
    let uri: http::Uri = raw.parse().map_err(|_| "is not a valid URL")?;
    let https = match uri.scheme_str() {
        Some("https") => true,
        Some("http") if loopback_allowed() => false,
        _ => return Err("must use https"),
    };
    let authority = uri.authority().ok_or("must name a host")?;
    if authority.as_str().contains('@') {
        return Err("must not contain user info");
    }
    let host = authority.host().to_owned();
    if host.is_empty() {
        return Err("must name a host");
    }
    let port = authority.port_u16().unwrap_or(if https { 443 } else { 80 });
    Ok(WebhookUrl {
        uri,
        host,
        port,
        https,
    })
}

/// The URL without its query string, for audit rows and logs (trap 20: a query may carry
/// a receiver token).
pub fn redact_url(raw: &str) -> String {
    match raw.parse::<http::Uri>() {
        Ok(uri) => {
            let scheme = uri.scheme_str().unwrap_or("https");
            let authority = uri
                .authority()
                .map(|a| a.as_str().rsplit('@').next().unwrap_or("").to_owned())
                .unwrap_or_default();
            let query = if uri.query().is_some() { "?…" } else { "" };
            format!("{scheme}://{authority}{}{query}", uri.path())
        }
        Err(_) => "[invalid url]".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_math() {
        assert!(v4_in(Ipv4Addr::new(172, 31, 255, 255), [172, 16, 0, 0], 12));
        assert!(!v4_in(Ipv4Addr::new(172, 32, 0, 0), [172, 16, 0, 0], 12));
        assert!(v6_in(
            "fdff::1".parse().unwrap(),
            "fc00::".parse().unwrap(),
            7
        ));
    }

    #[test]
    fn url_shape() {
        assert!(parse_webhook_url("https://his.example.org/hook?x=1").is_ok());
        assert_eq!(
            parse_webhook_url("https://u:p@his.example.org/").unwrap_err(),
            "must not contain user info"
        );
        assert_eq!(parse_webhook_url("ftp://x/").unwrap_err(), "must use https");
        assert_eq!(
            redact_url("https://u:p@his.example.org/hook?token=abc"),
            "https://his.example.org/hook?…"
        );
    }
}
