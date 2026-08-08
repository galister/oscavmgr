use anyhow::{Context, Result};
use log::{info, warn};
use mdns_sd::{IfKind, ServiceDaemon, ServiceInfo};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::Arc,
    thread,
    time::Duration,
};

const SERVICE_NAME: &str = "OscAvMgr";
const HOSTNAME: &str = "oscavmgr.local.";
const OSC_SERVICE: &str = "_osc._udp.local.";
const OSCQUERY_SERVICE: &str = "_oscjson._tcp.local.";
const FIRST_HTTP_PORT: u16 = 9402;
const MAX_REQUEST_LINE_LEN: u64 = 8192;

/// keeps mdns daemon alive while http server runs on separate thread
pub struct OscQueryAdvert {
    _mdns: ServiceDaemon,
}

impl OscQueryAdvert {
    pub fn new(osc_port: u16) -> Result<Self> {
        // mdns-sd advertises non-loopback interfaces → must also accept requests there
        let listener = bind_first_available(FIRST_HTTP_PORT)
            .context("could not bind an OSCQuery HTTP listener")?;
        let http_port = listener
            .local_addr()
            .context("could not read the OSCQuery HTTP listener address")?
            .port();

        let mdns = ServiceDaemon::new().context("could not start the mDNS daemon")?;
        mdns.disable_interface(IfKind::IPv6)
            .context("could not disable IPv6 OSCQuery advertisements")?;
        register_service(&mdns, OSC_SERVICE, osc_port)?;
        register_service(&mdns, OSCQUERY_SERVICE, http_port)?;

        let host_info = Arc::new(host_info_json(osc_port));
        if let Err(e) = thread::Builder::new()
            .name("oscquery-http".into())
            .spawn(move || serve(listener, host_info))
        {
            let _ = mdns.shutdown();
            return Err(e).context("could not start the OSCQuery HTTP thread");
        }

        info!(
            "Advertising {} through OSCQuery on TCP port {} (OSC port {})",
            SERVICE_NAME, http_port, osc_port
        );

        Ok(Self { _mdns: mdns })
    }
}

fn bind_first_available(first_port: u16) -> std::io::Result<TcpListener> {
    let mut last_error = None;
    for port in first_port..=u16::MAX {
        match TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)) {
            Ok(listener) => return Ok(listener),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => last_error = Some(e),
            Err(e) => return Err(e),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "no OSCQuery HTTP ports are available",
        )
    }))
}

fn register_service(mdns: &ServiceDaemon, service_type: &str, port: u16) -> Result<()> {
    let service = ServiceInfo::new(service_type, SERVICE_NAME, HOSTNAME, (), port, None)
        .with_context(|| format!("could not create the {service_type} service record"))?
        .enable_addr_auto();

    mdns.register(service)
        .with_context(|| format!("could not advertise {service_type}"))
}

fn serve(listener: TcpListener, host_info: Arc<Vec<u8>>) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => handle_connection(stream, &host_info),
            Err(e) => warn!("Failed to accept an OSCQuery connection: {e}"),
        }
    }
}

fn handle_connection(mut stream: TcpStream, host_info: &[u8]) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut request_line = String::new();
    let mut reader = BufReader::new(&stream).take(MAX_REQUEST_LINE_LEN);

    if reader.read_line(&mut request_line).is_err() || !request_line.ends_with('\n') {
        write_response(&mut stream, "400 Bad Request", "text/plain", b"Bad request");
        return;
    }

    let Some(target) = request_target(&request_line) else {
        write_response(&mut stream, "400 Bad Request", "text/plain", b"Bad request");
        return;
    };

    if target.contains("HOST_INFO") {
        write_response(&mut stream, "200 OK", "application/json", host_info);
        return;
    }

    let path = target.split('?').next().unwrap_or(target);
    let Some(node) = ROOT.find(path) else {
        write_response(
            &mut stream,
            "404 Not Found",
            "text/plain",
            b"OSC Path not found",
        );
        return;
    };

    match serde_json::to_vec(node) {
        Ok(body) => write_response(&mut stream, "200 OK", "application/json", &body),
        Err(e) => {
            warn!("Failed to serialize OSCQuery node {path}: {e}");
            write_response(
                &mut stream,
                "500 Internal Server Error",
                "text/plain",
                b"Internal server error",
            );
        }
    }
}

fn request_target(request_line: &str) -> Option<&str> {
    let mut parts = request_line.split_whitespace();
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("GET"), Some(target), Some(version), None) if version.starts_with("HTTP/") => {
            Some(target)
        }
        _ => None,
    }
}

fn write_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nPragma: no-cache\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

fn host_info_json(osc_port: u16) -> Vec<u8> {
    #[derive(Serialize)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    struct HostInfo {
        name: &'static str,
        extensions: BTreeMap<&'static str, bool>,
        osc_ip: &'static str,
        osc_port: u16,
        osc_transport: &'static str,
    }

    let extensions = BTreeMap::from([
        ("ACCESS", true),
        ("CLIPMODE", false),
        ("RANGE", true),
        ("TYPE", true),
        ("VALUE", true),
    ]);

    serde_json::to_vec(&HostInfo {
        name: SERVICE_NAME,
        extensions,
        osc_ip: "127.0.0.1",
        osc_port,
        osc_transport: "UDP",
    })
    .expect("serializing static OSCQuery host info cannot fail")
}

#[derive(Serialize)]
struct OscQueryNode {
    #[serde(rename = "DESCRIPTION", skip_serializing_if = "Option::is_none")]
    description: Option<&'static str>,
    #[serde(rename = "FULL_PATH")]
    full_path: &'static str,
    #[serde(rename = "ACCESS")]
    access: u8,
    #[serde(rename = "CONTENTS", skip_serializing_if = "BTreeMap::is_empty")]
    contents: BTreeMap<&'static str, OscQueryNode>,
    #[serde(rename = "TYPE", skip_serializing_if = "Option::is_none")]
    osc_type: Option<&'static str>,
}

impl OscQueryNode {
    fn branch(full_path: &'static str, contents: BTreeMap<&'static str, Self>) -> Self {
        Self {
            description: None,
            full_path,
            access: 0,
            contents,
            osc_type: None,
        }
    }

    fn endpoint(full_path: &'static str, osc_type: &'static str) -> Self {
        Self {
            description: None,
            full_path,
            access: 2,
            contents: BTreeMap::new(),
            osc_type: Some(osc_type),
        }
    }

    fn find(&self, path: &str) -> Option<&Self> {
        if path == "/" {
            return Some(self);
        }
        if !path.starts_with('/') || path.ends_with('/') {
            return None;
        }

        let mut node = self;
        for part in path[1..].split('/') {
            node = node.contents.get(part)?;
        }
        Some(node)
    }
}

static ROOT: Lazy<OscQueryNode> = Lazy::new(|| {
    let avatar = OscQueryNode::branch(
        "/avatar",
        BTreeMap::from([(
            "parameters",
            OscQueryNode::endpoint("/avatar/parameters", "b"),
        )]),
    );

    let vrsystem = OscQueryNode::branch(
        "/tracking/vrsystem",
        BTreeMap::from([
            (
                "head",
                OscQueryNode::branch(
                    "/tracking/vrsystem/head",
                    BTreeMap::from([(
                        "pose",
                        OscQueryNode::endpoint("/tracking/vrsystem/head/pose", "ffffff"),
                    )]),
                ),
            ),
            (
                "leftwrist",
                OscQueryNode::branch(
                    "/tracking/vrsystem/leftwrist",
                    BTreeMap::from([(
                        "pose",
                        OscQueryNode::endpoint("/tracking/vrsystem/leftwrist/pose", "ffffff"),
                    )]),
                ),
            ),
            (
                "rightwrist",
                OscQueryNode::branch(
                    "/tracking/vrsystem/rightwrist",
                    BTreeMap::from([(
                        "pose",
                        OscQueryNode::endpoint("/tracking/vrsystem/rightwrist/pose", "ffffff"),
                    )]),
                ),
            ),
        ]),
    );

    let tracking = OscQueryNode::branch("/tracking", BTreeMap::from([("vrsystem", vrsystem)]));

    OscQueryNode {
        description: Some("root node"),
        full_path: "/",
        access: 0,
        contents: BTreeMap::from([("avatar", avatar), ("tracking", tracking)]),
        osc_type: None,
    }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_info_matches_vrcadvert() {
        let value: serde_json::Value = serde_json::from_slice(&host_info_json(9002)).unwrap();

        assert_eq!(value["NAME"], "OscAvMgr");
        assert_eq!(value["OSC_IP"], "127.0.0.1");
        assert_eq!(value["OSC_PORT"], 9002);
        assert_eq!(value["OSC_TRANSPORT"], "UDP");
        assert_eq!(value["EXTENSIONS"]["ACCESS"], true);
        assert_eq!(value["EXTENSIONS"]["CLIPMODE"], false);
    }

    #[test]
    fn advertises_avatar_and_tracking_endpoints() {
        let avatar = ROOT.find("/avatar/parameters").unwrap();
        assert_eq!(avatar.access, 2);
        assert_eq!(avatar.osc_type, Some("b"));

        for tracker in ["head", "leftwrist", "rightwrist"] {
            let path = format!("/tracking/vrsystem/{tracker}/pose");
            let endpoint = ROOT.find(&path).unwrap();
            assert_eq!(endpoint.access, 2);
            assert_eq!(endpoint.osc_type, Some("ffffff"));
        }
    }

    #[test]
    fn parses_only_get_requests() {
        assert_eq!(
            request_target("GET /?HOST_INFO HTTP/1.1\r\n"),
            Some("/?HOST_INFO")
        );
        assert_eq!(request_target("POST / HTTP/1.1\r\n"), None);
        assert_eq!(request_target("not HTTP"), None);
    }

    #[test]
    fn selects_the_next_available_http_port() {
        let occupied = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
        let occupied_port = occupied.local_addr().unwrap().port();
        if occupied_port == u16::MAX {
            return;
        }

        let selected = bind_first_available(occupied_port).unwrap();
        assert!(selected.local_addr().unwrap().port() > occupied_port);
    }
}
