//! Wire evidence uses HTTP over loopback with a named synthetic HTTPS Origin.
//! It does not establish TLS, browser cookie enforcement, or provider login.

use std::{fmt::Write as _, net::SocketAddr, time::Duration};

use axum::Router;
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

pub const TRUSTED_ORIGIN: &str = "https://accounts-fixture.tabula.invalid";
pub const SESSION_COOKIE: &str = "__Host-tabula_session";
pub const PREAUTH_COOKIE: &str = "__Host-tabula_preauth";

#[derive(Debug)]
pub struct WireServer {
    address: SocketAddr,
    task: JoinHandle<()>,
}

impl WireServer {
    pub async fn start(router: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { address, task }
    }

    pub async fn request(
        &self,
        method: &str,
        target: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> WireResponse {
        let mut raw = format!(
            "{method} {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n",
            self.address,
            body.len()
        );
        for (name, value) in headers {
            write!(raw, "{name}: {value}\r\n").unwrap();
        }
        raw.push_str("\r\n");
        raw.push_str(body);
        self.raw_request(&raw).await
    }

    pub async fn raw_request(&self, raw: &str) -> WireResponse {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut stream = TcpStream::connect(self.address).await.unwrap();
            stream.write_all(raw.as_bytes()).await.unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).await.unwrap();
            WireResponse::parse(&bytes)
        })
        .await
        .expect("loopback HTTP response timed out")
    }
}

impl Drop for WireServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Debug)]
pub struct WireResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl WireResponse {
    fn parse(bytes: &[u8]) -> Self {
        let separator = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("HTTP response has a header separator");
        let header = std::str::from_utf8(&bytes[..separator]).unwrap();
        let mut lines = header.split("\r\n");
        let status = lines
            .next()
            .unwrap()
            .split_ascii_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let headers = lines
            .map(|line| {
                let (name, value) = line.split_once(':').unwrap();
                (name.to_ascii_lowercase(), value.trim().to_owned())
            })
            .collect::<Vec<_>>();
        let mut body = bytes[separator + 4..].to_vec();
        if headers
            .iter()
            .any(|(name, value)| name == "transfer-encoding" && value == "chunked")
        {
            body = decode_chunks(&body);
        }
        Self {
            status,
            headers,
            body: String::from_utf8(body).unwrap(),
        }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn headers(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }

    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).expect("response has JSON body")
    }

    pub fn assert_no_store(&self) {
        assert_eq!(self.header("cache-control"), Some("no-store"), "{self:?}");
        assert!(self.header("access-control-allow-origin").is_none());
        assert!(self.header("access-control-allow-credentials").is_none());
    }

    pub fn assert_no_cookie(&self) {
        assert!(self.headers("set-cookie").is_empty(), "{self:?}");
    }

    pub fn assert_excludes(&self, secrets: &[&str]) {
        for secret in secrets {
            assert!(
                !self.body.contains(secret),
                "private value in response body"
            );
        }
    }

    pub fn cookie(&self, name: &str) -> Option<String> {
        self.headers("set-cookie").into_iter().find_map(|header| {
            let pair = header.split(';').next().unwrap();
            let (candidate, _) = pair.split_once('=')?;
            (candidate == name).then(|| pair.to_owned())
        })
    }
}

fn decode_chunks(mut bytes: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::new();
    loop {
        let line_end = bytes
            .windows(2)
            .position(|window| window == b"\r\n")
            .expect("chunk has a length line");
        let size_text = std::str::from_utf8(&bytes[..line_end]).unwrap();
        let size = usize::from_str_radix(size_text.split(';').next().unwrap(), 16).unwrap();
        bytes = &bytes[line_end + 2..];
        if size == 0 {
            return decoded;
        }
        decoded.extend_from_slice(&bytes[..size]);
        assert_eq!(&bytes[size..size + 2], b"\r\n");
        bytes = &bytes[size + 2..];
    }
}

pub fn assert_session_cookie(header: &str, name: &str) {
    let attributes = header.split(';').map(str::trim).collect::<Vec<_>>();
    assert_eq!(attributes.len(), 5, "unexpected cookie attribute: {header}");
    assert!(attributes[0].starts_with(&format!("{name}=")));
    for expected in ["Secure", "HttpOnly", "SameSite=Lax", "Path=/"] {
        assert!(
            attributes.contains(&expected),
            "missing {expected}: {header}"
        );
    }
    for forbidden in ["domain", "max-age", "expires"] {
        assert!(!header.to_ascii_lowercase().contains(forbidden));
    }
}
