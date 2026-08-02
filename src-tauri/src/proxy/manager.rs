use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::upgrade::OnUpgrade;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioIo;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Arc;
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

const DEFAULT_PORT: u16 = 80;
const FALLBACK_PORT: u16 = 8787;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxyStatus {
    pub enabled: bool,
    pub listen_port: Option<u16>,
    pub hosts_active: bool,
}

pub struct ProxyManager {
    routes: Arc<RwLock<HashMap<String, u16>>>,
    listen_port: AtomicU16,
    running: AtomicBool,
    hosts_active: AtomicBool,
    shutdown_tx: parking_lot::Mutex<Option<oneshot::Sender<()>>>,
}

impl ProxyManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            routes: Arc::new(RwLock::new(HashMap::new())),
            listen_port: AtomicU16::new(0),
            running: AtomicBool::new(false),
            hosts_active: AtomicBool::new(false),
            shutdown_tx: parking_lot::Mutex::new(None),
        })
    }

    pub fn status(&self) -> ProxyStatus {
        let port = self.listen_port.load(Ordering::SeqCst);
        ProxyStatus {
            enabled: self.running.load(Ordering::SeqCst),
            listen_port: if port == 0 { None } else { Some(port) },
            hosts_active: self.hosts_active.load(Ordering::SeqCst),
        }
    }

    pub fn set_hosts_active(&self, active: bool) {
        self.hosts_active.store(active, Ordering::SeqCst);
    }

    pub fn set_routes(&self, routes: HashMap<String, u16>) {
        *self.routes.write() = routes;
    }

    pub fn stop(&self) {
        if let Some(tx) = self.shutdown_tx.lock().take() {
            let _ = tx.send(());
        }
        self.running.store(false, Ordering::SeqCst);
        self.listen_port.store(0, Ordering::SeqCst);
    }

    /// Start (or restart) the reverse proxy.
    /// `forced_port`: when set, only that port is tried. Otherwise: 80 then 8787.
    pub async fn start(self: &Arc<Self>, forced_port: Option<u16>) -> Result<u16> {
        self.stop();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let ports: Vec<u16> = match forced_port {
            Some(p) => vec![p],
            None => vec![DEFAULT_PORT, FALLBACK_PORT],
        };

        let mut last_err = None;
        for port in ports {
            match self.spawn_on(port).await {
                Ok(bound) => {
                    self.running.store(true, Ordering::SeqCst);
                    self.listen_port.store(bound, Ordering::SeqCst);
                    println!("[dev-tray] alias proxy listening on http://127.0.0.1:{bound}");
                    return Ok(bound);
                }
                Err(err) => {
                    eprintln!("[dev-tray] proxy bind :{port} failed: {err:#}");
                    last_err = Some(err);
                }
            }
        }

        anyhow::bail!(
            "could not start alias proxy: {}",
            last_err
                .map(|e| e.to_string())
                .unwrap_or_else(|| "no ports tried".into())
        )
    }

    async fn spawn_on(self: &Arc<Self>, port: u16) -> Result<u16> {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("failed to bind proxy on {addr}"))?;
        let bound = listener.local_addr()?.port();

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        *self.shutdown_tx.lock() = Some(shutdown_tx);

        let routes = Arc::clone(&self.routes);
        let app = Router::new().fallback(any(move |req: Request| {
            let routes = Arc::clone(&routes);
            async move { proxy_request(routes, req).await }
        }));

        tauri::async_runtime::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            if let Err(err) = server.await {
                eprintln!("[dev-tray] proxy server error: {err:#}");
            }
        });

        Ok(bound)
    }
}

fn host_key(host_header: &str) -> String {
    let host = host_header.trim().to_lowercase();
    let without_port = host.split(':').next().unwrap_or(&host);
    without_port
        .strip_suffix(".localhost")
        .unwrap_or(without_port)
        .to_string()
}

async fn proxy_request(routes: Arc<RwLock<HashMap<String, u16>>>, req: Request) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let key = host_key(host);
    let Some(port) = routes.read().get(&key).copied() else {
        return (
            StatusCode::NOT_FOUND,
            format!(
                "Dev Tray: unknown alias '{key}'. Add the project port in projects.json and Reload Config."
            ),
        )
            .into_response();
    };

    if is_websocket_upgrade(&req) {
        return match proxy_websocket(req, port).await {
            Ok(resp) => resp,
            Err(err) => (
                StatusCode::BAD_GATEWAY,
                format!("Dev Tray WebSocket proxy error: {err:#}"),
            )
                .into_response(),
        };
    }

    match proxy_http(req, port).await {
        Ok(resp) => resp,
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            format!("Dev Tray proxy error talking to upstream :{port}: {err:#}"),
        )
            .into_response(),
    }
}

fn is_websocket_upgrade(req: &Request) -> bool {
    let upgrade = req
        .headers()
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);
    let connection = req
        .headers()
        .get(header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_ascii_lowercase().contains("upgrade"))
        .unwrap_or(false);
    upgrade && connection
}

/// Vite on Windows often binds only `::1` for `localhost`; some stacks only `127.0.0.1`.
const UPSTREAM_HOSTS: &[&str] = &["127.0.0.1", "localhost", "[::1]"];

async fn connect_upstream(port: u16) -> Result<(TcpStream, &'static str)> {
    let mut last_err = None;
    for host in UPSTREAM_HOSTS {
        let addr = format!("{host}:{port}");
        match TcpStream::connect(&addr).await {
            Ok(stream) => {
                stream.set_nodelay(true).ok();
                return Ok((stream, host));
            }
            Err(err) => last_err = Some((addr, err)),
        }
    }
    let (addr, err) = last_err.expect("UPSTREAM_HOSTS is non-empty");
    Err(err).with_context(|| format!("failed to connect upstream ({addr} and fallbacks)"))
}

async fn proxy_http(req: Request, port: u16) -> Result<Response> {
    let (parts, body) = req.into_parts();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/")
        .to_string();

    let bytes = body
        .collect()
        .await
        .context("failed reading request body")?
        .to_bytes();

    let client: Client<HttpConnector, Full<Bytes>> =
        Client::builder(hyper_util::rt::TokioExecutor::new()).build(HttpConnector::new());

    let mut last_err = None;
    for host in UPSTREAM_HOSTS {
        let target_uri = format!("http://{host}:{port}{path_and_query}");
        let mut builder = hyper::Request::builder()
            .method(parts.method.clone())
            .uri(&target_uri);

        for (name, value) in parts.headers.iter() {
            if name == header::HOST {
                continue;
            }
            builder = builder.header(name, value);
        }
        // Host header without IPv6 brackets
        let host_header = host.trim_matches(|c| c == '[' || c == ']');
        builder = builder.header(header::HOST, format!("{host_header}:{port}"));

        let upstream_req = match builder.body(Full::new(bytes.clone())) {
            Ok(r) => r,
            Err(err) => {
                last_err = Some(anyhow::Error::new(err).context("failed to build upstream request"));
                continue;
            }
        };

        match client.request(upstream_req).await {
            Ok(upstream_res) => {
                let (upstream_parts, upstream_body) = upstream_res.into_parts();
                let mut response = Response::builder().status(upstream_parts.status);
                for (name, value) in upstream_parts.headers.iter() {
                    response = response.header(name, value);
                }
                let collected = upstream_body
                    .collect()
                    .await
                    .context("failed reading upstream body")?
                    .to_bytes();
                return Ok(response
                    .body(Body::from(collected))
                    .context("failed to build proxy response")?);
            }
            Err(err) => {
                last_err = Some(
                    anyhow::Error::new(err)
                        .context(format!("upstream request to {target_uri} failed")),
                );
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no upstream hosts to try")))
}

async fn proxy_websocket(mut req: Request, port: u16) -> Result<Response> {
    let on_upgrade = req
        .extensions_mut()
        .remove::<OnUpgrade>()
        .context("missing client OnUpgrade (is this a WebSocket request?)")?;

    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/")
        .to_string();

    let (upstream, host) = connect_upstream(port).await?;
    let host_header = host.trim_matches(|c| c == '[' || c == ']');

    let io = TokioIo::new(upstream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .context("WS upstream handshake failed")?;
    tauri::async_runtime::spawn(async move {
        let _ = conn.with_upgrades().await;
    });

    let mut builder = hyper::Request::builder()
        .method(req.method().clone())
        .uri(path_and_query);

    for (name, value) in req.headers().iter() {
        if name == header::HOST {
            continue;
        }
        builder = builder.header(name, value);
    }
    builder = builder.header(header::HOST, format!("{host_header}:{port}"));

    let upstream_req = builder
        .body(Full::new(Bytes::new()))
        .context("failed to build WS upstream request")?;

    let upstream_res = sender
        .send_request(upstream_req)
        .await
        .context("WS upstream upgrade request failed")?;

    if upstream_res.status() != StatusCode::SWITCHING_PROTOCOLS {
        let status = upstream_res.status();
        let body = upstream_res
            .into_body()
            .collect()
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();
        return Ok(Response::builder()
            .status(status)
            .body(Body::from(body))
            .context("failed to build WS error response")?);
    }

    let mut client_resp = Response::builder().status(StatusCode::SWITCHING_PROTOCOLS);
    for (name, value) in upstream_res.headers().iter() {
        if name == header::CONTENT_LENGTH {
            continue;
        }
        client_resp = client_resp.header(name, value);
    }

    let upstream_upgraded = hyper::upgrade::on(upstream_res)
        .await
        .context("failed to upgrade upstream connection")?;

    tauri::async_runtime::spawn(async move {
        match on_upgrade.await {
            Ok(client_upgraded) => {
                let mut client = TokioIo::new(client_upgraded);
                let mut upstream = TokioIo::new(upstream_upgraded);
                let _ = copy_bidirectional(&mut client, &mut upstream).await;
            }
            Err(err) => {
                eprintln!("[dev-tray] client WS upgrade failed: {err}");
            }
        }
    });

    Ok(client_resp
        .body(Body::empty())
        .context("failed to build WS switching response")?)
}
