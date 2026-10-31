//! owner: WP-I.7
//!
//! The outbound guard (DLV-3, trap 19) with the production policy: this binary never
//! relaxes it. Refused ranges per channel, a host name that resolves to loopback, and the
//! webhook client refusing before it connects. No database needed.

use std::net::IpAddr;

use asd_backend::interop::netguard::{self, Channel};
use asd_backend::interop::webhook;
use asd_backend::interop::ErrorClass;

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}

#[test]
fn refused_ranges_for_webhooks() {
    let refused = [
        "127.0.0.1",
        "127.255.255.254",
        "0.0.0.0",
        "10.0.0.1",
        "10.255.255.255",
        "172.16.0.1",
        "172.31.255.255",
        "192.168.1.10",
        "169.254.169.254", // cloud metadata
        "169.254.0.1",
        "100.64.0.1", // CGNAT
        "100.127.255.255",
        "224.0.0.1", // multicast
        "239.255.255.250",
        "255.255.255.255",
        "192.0.2.1", // documentation
        "198.18.0.1",
        "::1",
        "::",
        "fc00::1", // ULA
        "fd12:3456::1",
        "fe80::1", // link-local
        "febf::1",
        "ff02::1",          // multicast
        "::ffff:127.0.0.1", // IPv4-mapped loopback
        "::ffff:169.254.169.254",
        "::ffff:10.1.2.3",
        "64:ff9b::a9fe:a9fe", // NAT64 of 169.254.169.254
        "2001:db8::1",
    ];
    for a in refused {
        assert!(
            netguard::is_refused(ip(a), Channel::Http),
            "{a} must be refused"
        );
    }
    let allowed = [
        "8.8.8.8",
        "1.1.1.1",
        "172.32.0.1",
        "100.128.0.1",
        "192.169.0.1",
        "2606:4700:4700::1111",
        "::ffff:8.8.8.8",
    ];
    for a in allowed {
        assert!(
            !netguard::is_refused(ip(a), Channel::Http),
            "{a} must be allowed"
        );
    }
}

#[test]
fn sftp_may_reach_private_networks_but_not_loopback_or_metadata() {
    for a in [
        "10.0.0.5",
        "192.168.1.10",
        "172.20.0.2",
        "100.64.1.1",
        "fd00::5",
    ] {
        assert!(!netguard::is_refused(ip(a), Channel::Sftp), "{a}");
    }
    for a in [
        "127.0.0.1",
        "169.254.169.254",
        "::1",
        "fe80::1",
        "224.0.0.1",
        "0.0.0.0",
    ] {
        assert!(netguard::is_refused(ip(a), Channel::Sftp), "{a}");
    }
}

#[tokio::test]
async fn a_name_resolving_to_a_private_address_is_refused() {
    // `localhost` resolves to loopback on every test machine: the name is checked by what
    // it resolves to, not by its spelling.
    assert_eq!(
        netguard::resolve("localhost", 443, Channel::Http).await,
        Err(ErrorClass::AddressBlocked)
    );
    assert_eq!(
        netguard::resolve("169.254.169.254", 80, Channel::Http).await,
        Err(ErrorClass::AddressBlocked)
    );
    assert_eq!(
        netguard::resolve("[::1]", 443, Channel::Sftp).await,
        Err(ErrorClass::AddressBlocked)
    );
    assert_eq!(
        netguard::resolve("no-such-host.invalid", 443, Channel::Http).await,
        Err(ErrorClass::DnsFailed)
    );
}

#[tokio::test]
async fn webhook_client_refuses_before_connecting() {
    let refused = webhook::post("https://127.0.0.1:9/hook", b"k", "id", b"{}".to_vec())
        .await
        .unwrap_err();
    assert_eq!(refused.class, ErrorClass::AddressBlocked);
    let refused = webhook::post("https://localhost/hook", b"k", "id", b"{}".to_vec())
        .await
        .unwrap_err();
    assert_eq!(refused.class, ErrorClass::AddressBlocked);
    // Plain http is a config error under the production policy.
    let refused = webhook::post("http://8.8.8.8/hook", b"k", "id", b"{}".to_vec())
        .await
        .unwrap_err();
    assert_eq!(refused.class, ErrorClass::InvalidConfig);
}

#[test]
fn webhook_url_shape() {
    assert!(netguard::parse_webhook_url("https://his.example.org/hook?x=1").is_ok());
    for bad in [
        "http://his.example.org/hook",
        "https://user:pw@his.example.org/hook",
        "https:///nohost",
        "https://his.example.org/hook#frag",
        "file:///etc/passwd",
    ] {
        assert!(netguard::parse_webhook_url(bad).is_err(), "{bad}");
    }
    assert_eq!(
        netguard::redact_url("https://his.example.org/h?token=abc"),
        "https://his.example.org/h?…"
    );
}
