//! Actual loopback TCP around the isolated adapter. Cookie attachment and the
//! named HTTPS Origin are explicit test controls, not a browser or TLS oracle.

use std::{fmt::Write as _, net::SocketAddr, time::Duration};

use axum::Router;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

pub const TRUSTED_ORIGIN: &str = "https://accounts-fixture.tabula.invalid";
pub const SESSION_COOKIE: &str = "__Host-tabula_session";

#[derive(Debug)]
pub struct WireServer {
    address: SocketAddr,
    task: JoinHandle<()>,
}

impl WireServer {
    pub async fn start(router: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("isolated acceptance listener must start");
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { address, task }
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> WireResponse {
        let mut raw = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n",
            self.address,
            body.len()
        );
        for (name, value) in headers {
            write!(raw, "{name}: {value}\r\n").unwrap();
        }
        raw.push_str("\r\n");
        raw.push_str(body);
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut stream = TcpStream::connect(self.address)
                .await
                .expect("isolated adapter TCP connection must succeed");
            stream.write_all(raw.as_bytes()).await.unwrap();
            let mut bytes = Vec::new();
            // Retain headers even when a private body's guard fails and Hyper
            // closes/reset the stream after its successful response status.
            let read = stream.read_to_end(&mut bytes).await;
            WireResponse::parse(&bytes, read.is_err())
        })
        .await
        .expect("isolated adapter TCP exchange timed out")
    }
}

impl Drop for WireServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IncompleteBody;

#[derive(Debug)]
pub struct WireResponse {
    pub status: u16,
    headers: Vec<(String, String)>,
    pub body: Result<Vec<u8>, IncompleteBody>,
    received_body: Vec<u8>,
}

impl WireResponse {
    fn parse(bytes: &[u8], read_failed: bool) -> Self {
        let separator = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("isolated adapter must publish HTTP headers");
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
        let raw_body = &bytes[separator + 4..];
        let body = if read_failed {
            Err(IncompleteBody)
        } else if headers
            .iter()
            .any(|(name, value)| name == "transfer-encoding" && value == "chunked")
        {
            decode_chunks(raw_body)
        } else if headers.iter().any(|(name, value)| {
            name == "content-length"
                && value
                    .parse::<usize>()
                    .ok()
                    .is_none_or(|len| len != raw_body.len())
        }) {
            Err(IncompleteBody)
        } else {
            Ok(raw_body.to_vec())
        };
        Self {
            status,
            headers,
            body,
            received_body: raw_body.to_vec(),
        }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn cookie(&self, name: &str) -> Option<String> {
        self.headers.iter().find_map(|(header, value)| {
            if header != "set-cookie" {
                return None;
            }
            let pair = value.split(';').next().unwrap();
            let (candidate, _) = pair.split_once('=')?;
            (candidate == name).then(|| pair.to_owned())
        })
    }

    pub fn assert_no_store(&self) {
        assert_eq!(self.header("cache-control"), Some("no-store"));
        assert!(self.header("access-control-allow-origin").is_none());
        assert!(self.header("access-control-allow-credentials").is_none());
    }

    pub fn assert_received_excludes(&self, private_value: &str) {
        assert!(
            !self
                .received_body
                .windows(private_value.len())
                .any(|bytes| bytes == private_value.as_bytes()),
            "private bytes were received despite failed publication"
        );
    }
}

fn decode_chunks(mut bytes: &[u8]) -> Result<Vec<u8>, IncompleteBody> {
    let mut decoded = Vec::new();
    loop {
        let line_end = bytes
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or(IncompleteBody)?;
        let size_text = std::str::from_utf8(&bytes[..line_end]).map_err(|_| IncompleteBody)?;
        let size = usize::from_str_radix(size_text.split(';').next().unwrap(), 16)
            .map_err(|_| IncompleteBody)?;
        bytes = &bytes[line_end + 2..];
        if size == 0 {
            return if bytes == b"\r\n" {
                Ok(decoded)
            } else {
                Err(IncompleteBody)
            };
        }
        let suffix = bytes.get(size..).ok_or(IncompleteBody)?;
        if !suffix.starts_with(b"\r\n") {
            return Err(IncompleteBody);
        }
        decoded.extend_from_slice(&bytes[..size]);
        bytes = &suffix[2..];
    }
}

#[test]
fn incomplete_chunked_success_is_a_body_failure() {
    let response = WireResponse::parse(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n",
        false,
    );
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Err(IncompleteBody));
    assert_eq!(decode_chunks(b"2\r\n{}\r\n0\r\n\r\n"), Ok(b"{}".to_vec()));
}
