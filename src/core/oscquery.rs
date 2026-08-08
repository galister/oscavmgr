use anyhow::{bail, Context, Result};
use hickory_proto::{
    op::{Message, MessageType, OpCode},
    rr::{
        rdata::{A, PTR, SRV, TXT},
        Name, RData, Record,
    },
    serialize::binary::{BinDecodable, BinEncodable},
};
use log::{info, warn};
use once_cell::sync::Lazy;
use serde::Serialize;
use socket2::{Domain, Protocol, SockRef, Socket, Type};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream, UdpSocket},
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

const SERVICE_NAME: &str = "OscAvMgr";
const OSC_HOSTNAME: &str = "OscAvMgr.osc.local.";
const OSCQUERY_HOSTNAME: &str = "OscAvMgr.oscjson.local.";
const OSC_SERVICE: &str = "_osc._udp.local.";
const OSCQUERY_SERVICE: &str = "_oscjson._tcp.local.";
const FIRST_HTTP_PORT: u16 = 9402;
const MAX_REQUEST_LINE_LEN: u64 = 8192;
const MDNS_PORT: u16 = 5353;
const MDNS_ADDRESS: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const MDNS_RECORD_TTL: u32 = 4500;
const MDNS_HOST_TTL: u32 = 120;
const SERVICE_ENUMERATION: &str = "_services._dns-sd._udp.local.";

pub struct OscQueryAdvert {
    _http_thread: thread::JoinHandle<()>,
    _mdns_thread: thread::JoinHandle<()>,
}

impl OscQueryAdvert {
    pub fn new(osc_port: u16) -> Result<Self> {
        let listener = bind_first_available(FIRST_HTTP_PORT)
            .context("could not bind an OSCQuery HTTP listener")?;
        let http_port = listener
            .local_addr()
            .context("could not read the OSCQuery HTTP listener address")?
            .port();

        let mdns_socket = mdns_socket()?;
        let services = vec![
            MdnsService::new(OSC_SERVICE, OSC_HOSTNAME, osc_port)?,
            MdnsService::new(OSCQUERY_SERVICE, OSCQUERY_HOSTNAME, http_port)?,
        ];

        let host_info = Arc::new(host_info_json(osc_port));
        let http_thread = thread::Builder::new()
            .name("oscquery-http".into())
            .spawn(move || serve(listener, host_info))
            .context("could not start the OSCQuery HTTP thread")?;
        let mdns_thread = thread::Builder::new()
            .name("oscquery-mdns".into())
            .spawn(move || serve_mdns(mdns_socket, services))
            .context("could not start the OSCQuery mDNS thread")?;

        info!(
            "Advertising {} through OSCQuery on TCP port {} (OSC port {})",
            SERVICE_NAME, http_port, osc_port
        );

        Ok(Self {
            _http_thread: http_thread,
            _mdns_thread: mdns_thread,
        })
    }
}

struct MdnsSocket {
    socket: UdpSocket,
    interfaces: Vec<Ipv4Addr>,
}

fn mdns_socket() -> Result<MdnsSocket> {
    let mut interfaces = match if_addrs::get_if_addrs() {
        Ok(interfaces) => interfaces
            .into_iter()
            .filter_map(|interface| match interface.addr {
                if_addrs::IfAddr::V4(address)
                    if !address.ip.is_loopback() && address.broadcast.is_some() =>
                {
                    Some(address.ip)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>(),
        Err(e) => {
            warn!("Could not enumerate network interfaces for OSCQuery: {e}");
            BTreeSet::new()
        }
    };

    if interfaces.is_empty() {
        // INADDR_ANY asks the kernel to choose the default multicast interface
        interfaces.insert(Ipv4Addr::UNSPECIFIED);
        // keeps oscquery usable on hosts with only a loopback interface
        interfaces.insert(Ipv4Addr::LOCALHOST);
    }

    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .context("could not create the OSCQuery mDNS socket")?;
    socket
        .set_reuse_address(true)
        .context("could not enable address reuse for OSCQuery mDNS")?;
    #[cfg(target_family = "unix")]
    socket
        .set_reuse_port(true)
        .context("could not enable port reuse for OSCQuery mDNS")?;
    socket
        .bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, MDNS_PORT).into())
        .context("could not bind the OSCQuery mDNS socket")?;
    socket
        .set_multicast_loop_v4(true)
        .context("could not enable multicast loopback for OSCQuery mDNS")?;
    socket
        .set_multicast_ttl_v4(255)
        .context("could not set the OSCQuery mDNS multicast TTL")?;

    let mut joined = Vec::new();
    for interface in interfaces {
        match socket.join_multicast_v4(&MDNS_ADDRESS, &interface) {
            Ok(()) => joined.push(interface),
            Err(e) => warn!("Failed to join the mDNS group on {interface}: {e}"),
        }
    }
    if joined.is_empty() {
        bail!("could not join the mDNS group on any IPv4 interface");
    }

    Ok(MdnsSocket {
        socket: socket.into(),
        interfaces: joined,
    })
}

struct MdnsService {
    service_type: Name,
    instance_name: Name,
    hostname: Name,
    response: Vec<u8>,
}

impl MdnsService {
    fn new(service_type: &str, hostname: &str, port: u16) -> Result<Self> {
        let service_type = Name::from_ascii(service_type).context("invalid mDNS service type")?;
        let instance_name = Name::from_ascii(format!("{SERVICE_NAME}.{service_type}"))
            .context("invalid mDNS service instance name")?;
        let hostname = Name::from_ascii(hostname).context("invalid mDNS hostname")?;
        let response = service_response(&service_type, &instance_name, &hostname, port)?;

        Ok(Self {
            service_type,
            instance_name,
            hostname,
            response,
        })
    }

    fn matches(&self, name: &Name) -> bool {
        name == &self.service_type || name == &self.instance_name || name == &self.hostname
    }
}

fn service_response(
    service_type: &Name,
    instance_name: &Name,
    hostname: &Name,
    port: u16,
) -> Result<Vec<u8>> {
    let mut response = mdns_response();
    response.add_answer(Record::from_rdata(
        service_type.clone(),
        MDNS_RECORD_TTL,
        RData::PTR(PTR(instance_name.clone())),
    ));
    response.add_additional(Record::from_rdata(
        instance_name.clone(),
        MDNS_RECORD_TTL,
        RData::SRV(SRV::new(0, 0, port, hostname.clone())),
    ));
    response.add_additional(Record::from_rdata(
        instance_name.clone(),
        MDNS_RECORD_TTL,
        RData::TXT(TXT::new(vec!["txtvers=1".into()])),
    ));
    response.add_additional(Record::from_rdata(
        hostname.clone(),
        MDNS_HOST_TTL,
        RData::A(A(Ipv4Addr::LOCALHOST)),
    ));
    response
        .to_bytes()
        .context("could not encode an OSCQuery mDNS response")
}

fn enumeration_response(services: &[MdnsService]) -> Result<Vec<u8>> {
    let enumeration =
        Name::from_ascii(SERVICE_ENUMERATION).context("invalid DNS-SD service enumeration name")?;
    let mut response = mdns_response();
    for service in services {
        response.add_answer(Record::from_rdata(
            enumeration.clone(),
            MDNS_RECORD_TTL,
            RData::PTR(PTR(service.service_type.clone())),
        ));
    }
    response
        .to_bytes()
        .context("could not encode a DNS-SD service enumeration response")
}

static MESSAGE_ID: AtomicU16 = AtomicU16::new(42);

fn mdns_response() -> Message {
    let mut response = Message::response(MESSAGE_ID.fetch_add(1, Ordering::Relaxed), OpCode::Query);
    response.metadata.authoritative = true;
    response
}

fn serve_mdns(mdns: MdnsSocket, services: Vec<MdnsService>) {
    let enumeration_name = Name::from_ascii(SERVICE_ENUMERATION).unwrap();
    let enumeration = enumeration_response(&services).unwrap();

    announce_services(&mdns, &services);
    thread::sleep(Duration::from_secs(1));
    announce_services(&mdns, &services);

    let mut buffer = [0u8; 9000];
    loop {
        let (size, source) = match mdns.socket.recv_from(&mut buffer) {
            Ok(received) => received,
            Err(e) => {
                warn!("Failed to receive an OSCQuery mDNS packet: {e}");
                thread::sleep(Duration::from_millis(100));
                continue;
            }
        };

        let Ok(query) = Message::from_bytes(&buffer[..size]) else {
            continue;
        };
        if query.message_type != MessageType::Query {
            continue;
        }

        let mut matched = vec![false; services.len()];
        let mut enumerate = false;
        let mut qu_unicast = false;
        for question in query.queries.iter() {
            enumerate |= question.name() == &enumeration_name;
            qu_unicast |= question.mdns_unicast_response();
            for (index, service) in services.iter().enumerate() {
                matched[index] |= service.matches(question.name());
            }
        }

        let legacy_unicast = source.port() != MDNS_PORT;
        let destination = (legacy_unicast || qu_unicast).then_some(source);
        if enumerate {
            send_query_response(&mdns, &enumeration, &query, legacy_unicast, destination);
        }
        for (service, matched) in services.iter().zip(matched) {
            if matched {
                send_query_response(
                    &mdns,
                    &service.response,
                    &query,
                    legacy_unicast,
                    destination,
                );
            }
        }
    }
}

fn send_query_response(
    mdns: &MdnsSocket,
    response: &[u8],
    query: &Message,
    legacy_unicast: bool,
    destination: Option<std::net::SocketAddr>,
) {
    if legacy_unicast {
        match legacy_unicast_response(response, query) {
            Ok(response) => send_mdns_response(mdns, &response, destination),
            Err(e) => warn!("Failed to encode a legacy unicast mDNS response: {e:#}"),
        }
    } else {
        send_mdns_response(mdns, response, destination);
    }
}

fn legacy_unicast_response(response: &[u8], query: &Message) -> Result<Vec<u8>> {
    let mut response = Message::from_bytes(response)
        .context("could not decode the base OSCQuery mDNS response")?;

    response.metadata.id = query.id;

    for question in query.queries.iter() {
        let mut question = question.clone();
        question.set_mdns_unicast_response(false);
        response.add_query(question);
    }
    for record in response.answers.iter_mut() {
        record.ttl = record.ttl.min(10);
    }
    for record in response.additionals.iter_mut() {
        record.ttl = record.ttl.min(10);
    }

    response
        .to_bytes()
        .context("could not encode a legacy unicast OSCQuery mDNS response")
}

fn announce_services(mdns: &MdnsSocket, services: &[MdnsService]) {
    for service in services {
        send_mdns_response(mdns, &service.response, None);
    }
}

fn send_mdns_response(
    mdns: &MdnsSocket,
    response: &[u8],
    unicast_destination: Option<std::net::SocketAddr>,
) {
    if let Some(destination) = unicast_destination {
        if let Err(e) = mdns.socket.send_to(response, destination) {
            warn!("Failed to send a unicast OSCQuery mDNS response: {e}");
        }
        return;
    }

    let destination = SocketAddrV4::new(MDNS_ADDRESS, MDNS_PORT);
    for interface in &mdns.interfaces {
        let socket = SockRef::from(&mdns.socket);
        if let Err(e) = socket.set_multicast_if_v4(interface) {
            warn!("Failed to select mDNS interface {interface}: {e}");
        } else if let Err(e) = mdns.socket.send_to(response, destination) {
            warn!("Failed to send an OSCQuery mDNS response on {interface}: {e}");
        }
    }
}

fn bind_first_available(first_port: u16) -> std::io::Result<TcpListener> {
    let mut last_error = None;
    for port in first_port..=u16::MAX {
        match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
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
        log::warn!("404 Not Found on: {}", request_line);
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
            description: Some(""),
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
        BTreeMap::from([
            ("change", OscQueryNode::endpoint("/avatar/change", "s")),
            (
                "parameters",
                OscQueryNode::endpoint("/avatar/parameters", "b"),
            ),
        ]),
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
    fn advertises_avatar_and_tracking_endpoints() {
        let avatar_change = ROOT.find("/avatar/change").unwrap();
        assert_eq!(avatar_change.access, 2);
        assert_eq!(avatar_change.osc_type, Some("s"));

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
        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let occupied_port = occupied.local_addr().unwrap().port();
        if occupied_port == u16::MAX {
            return;
        }

        let selected = bind_first_available(occupied_port).unwrap();
        assert!(selected.local_addr().unwrap().port() > occupied_port);
    }
}
