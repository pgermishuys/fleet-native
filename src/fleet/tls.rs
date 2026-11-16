//! TLS for the hub's WebSocket (`wss://`), the same stack ureq uses for https calls: rustls with the `ring` provider and
//! the webpki root certificates (what Let's Encrypt, which `tailscale serve` uses, chains to). One verifier for both, never
//! a switch that turns verification off.
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// A hub connection's byte stream: the socket, or the socket inside TLS.
pub enum Stream {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Stream {
    /// `host_port` is `host:port`; `name` is what the certificate must be for. `secure` picks TLS.
    pub fn open(secure: bool, host_port: &str, name: &str) -> Result<Stream, String> {
        let tcp = TcpStream::connect(host_port).map_err(|e| e.to_string())?;
        let _ = tcp.set_nodelay(true);
        if !secure {
            return Ok(Stream::Plain(tcp));
        }
        let server = ServerName::try_from(name.to_string()).map_err(|e| format!("{name} isn't a name a certificate can be for: {e}"))?;
        let conn = ClientConnection::new(config(), server).map_err(|e| e.to_string())?;
        Ok(Stream::Tls(Box::new(StreamOwned::new(conn, tcp))))
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            Stream::Plain(s) => s.set_read_timeout(timeout),
            Stream::Tls(s) => s.sock.set_read_timeout(timeout),
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
        }
    }
}

fn config() -> Arc<ClientConfig> {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            #[cfg(test)]
            for der in test_roots::all() {
                roots.add(der).expect("a test certificate");
            }
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let config = ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .expect("ring supports the default TLS versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
            Arc::new(config)
        })
        .clone()
}

/// What a test trusts besides the public roots: its own certificate, added before the first connection. Only compiled
/// for tests, so the app can't be told to trust anything.
#[cfg(test)]
pub mod test_roots {
    use rustls::pki_types::CertificateDer;
    use std::sync::Mutex;

    static EXTRA: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());

    pub fn trust(der: &[u8]) {
        let mut all = EXTRA.lock().unwrap();
        if !all.iter().any(|d| d == der) {
            all.push(der.to_vec());
        }
    }

    pub(crate) fn all() -> Vec<CertificateDer<'static>> {
        EXTRA.lock().unwrap().iter().map(|d| CertificateDer::from(d.clone())).collect()
    }
}
