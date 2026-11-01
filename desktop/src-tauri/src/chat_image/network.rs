//! Public network validation, Cloudflare DoH fallback, and HTTPS image fetching.

use std::io::Read as _;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs as _};
use std::time::Duration;

use serde::Deserialize;
use url::{Host, Url};

pub(super) const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

const MAX_REDIRECTS: usize = 5;

const CLOUDFLARE_DOH_HOST: &str = "cloudflare-dns.com";
const CLOUDFLARE_BOOTSTRAP_IPS: &[IpAddr] = &[
    IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
    IpAddr::V4(Ipv4Addr::new(1, 0, 0, 1)),
    IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111)),
    IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1001)),
];

#[derive(Debug, Deserialize)]
struct CloudflareDnsResponse {
    #[serde(rename = "Status")]
    status: i32,
    #[serde(rename = "Answer")]
    answer: Option<Vec<CloudflareDnsAnswer>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CloudflareDnsAnswer {
    #[serde(rename = "type")]
    record_type: u16,
    data: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum DnsResolutionDecision {
    UseDirect(Vec<SocketAddr>),
    QueryDoH,
    Deny(String),
}

/// Whether `ip` falls within the 198.18.0.0/15 benchmarking range used by Fake-IP proxies.
pub(super) fn is_fake_ip_benchmark(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 198 && (o[1] == 18 || o[1] == 19)
        }
        IpAddr::V6(_) => false,
    }
}

/// Whether `ip` is a publicly routable IP address.
pub(super) fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_ipv4(v4),
        IpAddr::V6(v6) => is_public_ipv6(v6),
    }
}

pub(super) fn is_public_ipv4(v4: Ipv4Addr) -> bool {
    let o = v4.octets();
    !(o[0] == 0
        || o[0] == 10
        || (o[0] == 100 && (o[1] & 0xc0) == 64)
        || o[0] == 127
        || (o[0] == 169 && o[1] == 254)
        || (o[0] == 172 && (16..=31).contains(&o[1]))
        || (o[0] == 192 && o[1] == 0 && (o[2] == 0 || o[2] == 2))
        || (o[0] == 192 && o[1] == 168)
        || (o[0] == 198 && (o[1] == 18 || o[1] == 19 || (o[1] == 51 && o[2] == 100)))
        || (o[0] == 203 && o[1] == 0 && o[2] == 113)
        || o[0] >= 224)
}

pub(super) fn is_public_ipv6(v6: Ipv6Addr) -> bool {
    let s = v6.segments();
    // IPv4-mapped IPv6 (::ffff:0:0/96)
    if s[0..5] == [0; 5] && s[5] == 0xffff {
        let v4 = Ipv4Addr::new(
            (s[6] >> 8) as u8,
            (s[6] & 0xff) as u8,
            (s[7] >> 8) as u8,
            (s[7] & 0xff) as u8,
        );
        return is_public_ipv4(v4);
    }

    // Must be in Global Unicast 2000::/3
    if (s[0] & 0xe000) != 0x2000 {
        return false;
    }

    // Exclude IETF special 2001::/23 (covers Teredo 2001:0000::/32, benchmark 2001:2::/48)
    if s[0] == 0x2001 && (s[1] & 0xfe00) == 0 {
        return false;
    }
    // Exclude documentation 2001:db8::/32 and 3fff::/20
    if (s[0] == 0x2001 && s[1] == 0x0db8) || (s[0] == 0x3fff && (s[1] & 0xf000) == 0) {
        return false;
    }
    // Exclude deprecated 6to4 2002::/16
    if s[0] == 0x2002 {
        return false;
    }

    true
}

pub(super) fn evaluate_system_dns_answers(
    domain: &str,
    addrs: &[SocketAddr],
) -> DnsResolutionDecision {
    if addrs.is_empty() {
        return DnsResolutionDecision::Deny(format!("DNS returned no addresses for {domain}"));
    }

    let mut has_fake_ip = false;
    let mut has_other_non_public = false;
    let mut first_non_public = None;

    for sa in addrs {
        let ip = sa.ip();
        if is_fake_ip_benchmark(ip) {
            has_fake_ip = true;
        } else if !is_public_ip(ip) {
            has_other_non_public = true;
            if first_non_public.is_none() {
                first_non_public = Some(ip);
            }
        }
    }

    if has_other_non_public {
        return DnsResolutionDecision::Deny(format!(
            "DNS resolution returned non-public IP {} for {domain}",
            first_non_public.unwrap()
        ));
    }

    if has_fake_ip {
        return DnsResolutionDecision::QueryDoH;
    }

    DnsResolutionDecision::UseDirect(addrs.to_vec())
}

pub(super) fn parse_doh_answers(json_str: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    let resp: CloudflareDnsResponse = serde_json::from_str(json_str)
        .map_err(|e| format!("invalid Cloudflare DoH JSON: {e}"))?;

    if let Some(err) = resp.error {
        return Err(format!("Cloudflare DoH error: {err}"));
    }

    if resp.status != 0 {
        if resp.status == 3 {
            return Err("domain does not exist (NXDOMAIN)".to_string());
        }
        return Err(format!(
            "Cloudflare DoH returned DNS status code {}",
            resp.status
        ));
    }

    let mut addrs = Vec::new();
    if let Some(answers) = resp.answer {
        for ans in answers {
            if ans.record_type == 1 || ans.record_type == 28 {
                if let Ok(ip) = ans.data.trim().parse::<IpAddr>() {
                    if is_public_ip(ip) && !is_fake_ip_benchmark(ip) {
                        addrs.push(SocketAddr::new(ip, port));
                    }
                }
            }
        }
    }

    Ok(addrs)
}

fn query_cloudflare_doh_record(
    client: &reqwest::blocking::Client,
    domain: &str,
    record_type: &str,
    port: u16,
) -> Result<Vec<SocketAddr>, String> {
    let url = format!("https://{CLOUDFLARE_DOH_HOST}/dns-query?name={domain}&type={record_type}");
    let response = client
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/dns-json")
        .header(
            "User-Agent",
            concat!("NPi-Desktop/", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .map_err(|e| format!("Cloudflare DoH request failed for {domain}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Cloudflare DoH HTTP error status: {}",
            response.status()
        ));
    }

    let mut body = String::new();
    response
        .take(64 * 1024)
        .read_to_string(&mut body)
        .map_err(|e| format!("failed to read DoH response: {e}"))?;

    parse_doh_answers(&body, port)
}

fn resolve_via_cloudflare_doh(domain: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    let bootstrap_addrs: Vec<SocketAddr> = CLOUDFLARE_BOOTSTRAP_IPS
        .iter()
        .map(|ip| SocketAddr::new(*ip, 443))
        .collect();

    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .resolve_to_addrs(CLOUDFLARE_DOH_HOST, &bootstrap_addrs)
        .build()
        .map_err(|e| format!("failed to build DoH client: {e}"))?;

    let mut addrs = query_cloudflare_doh_record(&client, domain, "A", port)?;
    if addrs.is_empty() {
        addrs = query_cloudflare_doh_record(&client, domain, "AAAA", port)?;
    }

    if addrs.is_empty() {
        return Err(format!(
            "Cloudflare DoH returned no public IP addresses for {domain}"
        ));
    }

    Ok(addrs)
}

fn resolve_domain_addrs(domain: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    let resolved = (domain, port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for {domain}: {e}"))?
        .collect::<Vec<_>>();

    match evaluate_system_dns_answers(domain, &resolved) {
        DnsResolutionDecision::UseDirect(addrs) => Ok(addrs),
        DnsResolutionDecision::QueryDoH => resolve_via_cloudflare_doh(domain, port),
        DnsResolutionDecision::Deny(reason) => Err(reason),
    }
}

pub(super) fn detect_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else if bytes.starts_with(b"\x00\x00\x01\x00") {
        Some("image/x-icon")
    } else if bytes.len() >= 12
        && &bytes[4..8] == b"ftyp"
        && (&bytes[8..12] == b"avif" || &bytes[8..12] == b"avis")
    {
        Some("image/avif")
    } else if is_svg(bytes) {
        Some("image/svg+xml")
    } else {
        None
    }
}

fn is_svg(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok_and(|s| {
        let trimmed = s.trim_start();
        trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && trimmed.contains("<svg"))
    })
}

pub(super) fn fetch_public_https_image(
    url_str: &str,
) -> Result<(Vec<u8>, &'static str), String> {
    let mut current_url = Url::parse(url_str).map_err(|e| format!("invalid image URL: {e}"))?;

    for hop in 0..=MAX_REDIRECTS {
        if current_url.scheme() != "https" {
            return Err("only HTTPS URLs are permitted".to_string());
        }
        if !current_url.username().is_empty() || current_url.password().is_some() {
            return Err("URL userinfo credentials are not permitted".to_string());
        }

        let port = current_url.port_or_known_default().unwrap_or(443);
        let (host_str, addrs) = match current_url.host() {
            Some(Host::Ipv4(v4)) => {
                if !is_public_ipv4(v4) {
                    return Err(format!("non-public IP address {v4} is denied"));
                }
                (v4.to_string(), vec![SocketAddr::new(IpAddr::V4(v4), port)])
            }
            Some(Host::Ipv6(v6)) => {
                if !is_public_ipv6(v6) {
                    return Err(format!("non-public IP address {v6} is denied"));
                }
                (v6.to_string(), vec![SocketAddr::new(IpAddr::V6(v6), port)])
            }
            Some(Host::Domain(domain)) => {
                let addrs = resolve_domain_addrs(domain, port)?;
                (domain.to_string(), addrs)
            }
            None => return Err("URL missing host".to_string()),
        };

        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .resolve_to_addrs(&host_str, &addrs)
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        let response = client
            .get(current_url.as_str())
            .header(
                "User-Agent",
                concat!("NPi-Desktop/", env!("CARGO_PKG_VERSION")),
            )
            .send()
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        let status = response.status();
        if status.is_redirection() {
            if hop == MAX_REDIRECTS {
                return Err("too many HTTP redirects".to_string());
            }
            let loc = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or_else(|| {
                    format!("HTTP redirect {} missing Location header", status.as_u16())
                })?
                .to_str()
                .map_err(|_| "invalid redirect Location header".to_string())?;

            current_url = current_url
                .join(loc)
                .map_err(|e| format!("invalid redirect target: {e}"))?;
            continue;
        }

        if !status.is_success() {
            return Err(format!("HTTP error status: {status}"));
        }

        if let Some(cl) = response.content_length() {
            if cl > MAX_IMAGE_BYTES {
                return Err(format!("image size ({cl} bytes) exceeds maximum 8 MiB"));
            }
        }

        let mut bytes = Vec::new();
        response
            .take(MAX_IMAGE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("failed to read response body: {e}"))?;

        if bytes.len() as u64 > MAX_IMAGE_BYTES {
            return Err("image size exceeds maximum 8 MiB".to_string());
        }

        let mime = detect_image_mime(&bytes)
            .ok_or_else(|| "unsupported image format or invalid signature".to_string())?;

        if mime == "image/svg+xml" {
            return Err("remote SVG images are not permitted".to_string());
        }

        return Ok((bytes, mime));
    }

    Err("too many HTTP redirects".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_ip_benchmark_identification() {
        assert!(is_fake_ip_benchmark("198.18.0.1".parse().unwrap()));
        assert!(is_fake_ip_benchmark("198.18.1.170".parse().unwrap()));
        assert!(is_fake_ip_benchmark("198.19.255.255".parse().unwrap()));

        assert!(!is_fake_ip_benchmark("198.20.0.1".parse().unwrap()));
        assert!(!is_fake_ip_benchmark("192.168.1.1".parse().unwrap()));
        assert!(!is_fake_ip_benchmark("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn dns_resolution_decision_triggers_doh_only_for_pure_fake_ip() {
        let public_sa = SocketAddr::new("93.184.216.34".parse().unwrap(), 443);
        let fake_sa = SocketAddr::new("198.18.1.170".parse().unwrap(), 443);
        let private_sa = SocketAddr::new("192.168.1.5".parse().unwrap(), 443);
        let loopback_sa = SocketAddr::new("127.0.0.1".parse().unwrap(), 443);

        assert_eq!(
            evaluate_system_dns_answers("example.com", &[public_sa]),
            DnsResolutionDecision::UseDirect(vec![public_sa])
        );

        assert_eq!(
            evaluate_system_dns_answers("httpbin.org", &[fake_sa]),
            DnsResolutionDecision::QueryDoH
        );

        match evaluate_system_dns_answers("corp.internal", &[private_sa]) {
            DnsResolutionDecision::Deny(msg) => assert!(msg.contains("192.168.1.5")),
            other => panic!("expected Deny, got {other:?}"),
        }

        match evaluate_system_dns_answers("localhost", &[loopback_sa]) {
            DnsResolutionDecision::Deny(msg) => assert!(msg.contains("127.0.0.1")),
            other => panic!("expected Deny, got {other:?}"),
        }

        match evaluate_system_dns_answers("mixed.host", &[fake_sa, private_sa]) {
            DnsResolutionDecision::Deny(msg) => assert!(msg.contains("192.168.1.5")),
            other => panic!("expected Deny, got {other:?}"),
        }

        match evaluate_system_dns_answers("none.host", &[]) {
            DnsResolutionDecision::Deny(msg) => assert!(msg.contains("no addresses")),
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    #[test]
    fn doh_json_parser_extracts_public_ips_and_filters_private() {
        let json_ok = r#"{
            "Status": 0,
            "Answer": [
                { "name": "httpbin.org.", "type": 1, "data": "3.137.48.32" },
                { "name": "httpbin.org.", "type": 1, "data": "10.0.0.1" },
                { "name": "httpbin.org.", "type": 1, "data": "198.18.1.1" },
                { "name": "httpbin.org.", "type": 28, "data": "2606:4700:4700::1111" }
            ]
        }"#;

        let addrs = parse_doh_answers(json_ok, 443).unwrap();
        assert_eq!(addrs.len(), 2);
        assert_eq!(
            addrs[0],
            SocketAddr::new("3.137.48.32".parse().unwrap(), 443)
        );
        assert_eq!(
            addrs[1],
            SocketAddr::new("2606:4700:4700::1111".parse().unwrap(), 443)
        );

        let json_nxdomain = r#"{ "Status": 3, "Answer": [] }"#;
        assert!(parse_doh_answers(json_nxdomain, 443).is_err());

        let json_err = r#"{ "Status": 2, "error": "Server failure" }"#;
        assert!(parse_doh_answers(json_err, 443).is_err());
    }

    #[test]
    fn public_ipv4_accepts_public_addresses() {
        for ip in [
            "8.8.8.8",
            "1.1.1.1",
            "93.184.216.34",
            "140.82.121.4",
            "151.101.1.140",
        ] {
            assert!(
                is_public_ip(ip.parse().unwrap()),
                "{ip} should be accepted as public"
            );
        }
    }

    #[test]
    fn public_ipv4_rejects_private_and_reserved_addresses() {
        for ip in [
            "0.0.0.0",
            "10.0.0.1",
            "10.255.255.255",
            "100.64.0.1",
            "100.127.255.254",
            "127.0.0.1",
            "127.255.255.255",
            "169.254.169.254",
            "172.16.0.1",
            "172.31.255.255",
            "192.0.0.1",
            "192.0.2.1",
            "192.168.0.1",
            "192.168.255.255",
            "198.18.0.1",
            "198.19.255.255",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "239.255.255.255",
            "240.0.0.1",
            "255.255.255.255",
        ] {
            assert!(
                !is_public_ip(ip.parse().unwrap()),
                "{ip} should be rejected as non-public"
            );
        }
    }

    #[test]
    fn public_ipv6_accepts_global_unicast() {
        for ip in [
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
            "2a00:1450:4009:820::200e",
        ] {
            assert!(
                is_public_ip(ip.parse().unwrap()),
                "{ip} should be accepted as public"
            );
        }
    }

    #[test]
    fn public_ipv6_rejects_non_global_and_special() {
        for ip in [
            "::",
            "::1",
            "::2",
            "100::1",
            "2001:0::1",
            "2001:2::1",
            "2001:db8::1",
            "2002::1",
            "3fff::1",
            "fc00::1",
            "fd12:3456:789a::1",
            "fe80::1",
            "fec0::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "::ffff:192.168.1.1",
            "::ffff:169.254.1.1",
        ] {
            assert!(
                !is_public_ip(ip.parse().unwrap()),
                "{ip} should be rejected as non-public"
            );
        }
    }

    #[test]
    fn detects_supported_image_signatures() {
        assert_eq!(
            detect_image_mime(b"\x89PNG\r\n\x1a\n\x00\x00"),
            Some("image/png")
        );
        assert_eq!(detect_image_mime(b"\xff\xd8\xff\xe0"), Some("image/jpeg"));
        assert_eq!(detect_image_mime(b"GIF89a\x01\x00"), Some("image/gif"));
        assert_eq!(detect_image_mime(b"GIF87a\x01\x00"), Some("image/gif"));
        assert_eq!(
            detect_image_mime(b"RIFF\x00\x00\x00\x00WEBPVP8"),
            Some("image/webp")
        );
        assert_eq!(detect_image_mime(b"BM\x36\x00"), Some("image/bmp"));
        assert_eq!(
            detect_image_mime(b"\x00\x00\x01\x00\x01\x00"),
            Some("image/x-icon")
        );
        assert_eq!(
            detect_image_mime(b"\x00\x00\x00\x20ftypavif\x00\x00"),
            Some("image/avif")
        );
        assert_eq!(
            detect_image_mime(b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"),
            Some("image/svg+xml")
        );
        assert_eq!(
            detect_image_mime(
                b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"
            ),
            Some("image/svg+xml")
        );
        assert_eq!(detect_image_mime(b"random binary bytes"), None);
    }

    #[test]
    fn rejects_userinfo_and_non_https() {
        assert!(fetch_public_https_image("http://example.com/insecure.png").is_err());
        assert!(fetch_public_https_image("https://user:pass@example.com/img.png").is_err());
    }
}
