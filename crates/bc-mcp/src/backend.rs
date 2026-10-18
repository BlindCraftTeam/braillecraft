//! Where tool calls end up: a backend answering control-channel methods. `Remote` reaches a running desktop program
//! through its loopback control port (`braillecraft-desktop --control PORT`), newline-delimited JSON requests
//! {id, method, params, key} and replies {id, ok, result} or {id, ok: false, error}. The control key is read from
//! BRAILLECRAFT_CONTROL_KEY or from the key file the program writes for the port in the settings folder, and is
//! sent only to a loopback address.

use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

/// Something that answers control-channel methods.
pub trait Backend {
    /// Calls one method. Ok carries the result, Err the error message.
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String>;
    /// Whether a real program window is attached (screenshots, keys and clicks work).
    fn has_ui(&self) -> bool;
    /// A short description ("headless session", "connected to 127.0.0.1:7991").
    fn describe(&self) -> String;
}

/// The key file of the program's control port: control-<port>.key in the settings folder.
pub fn key_path(settings: &Path, port: u16) -> PathBuf {
    settings.join(format!("control-{port}.key"))
}

type KeyFinder = Box<dyn Fn(&str) -> Result<String, String> + Send>;

/// A running desktop program, reached through its loopback control port.
pub struct Remote {
    addr: String,
    conn: Option<(BufReader<TcpStream>, TcpStream)>,
    next_id: u64,
    send_key: bool,
    key: Option<String>,
    find_key: KeyFinder,
}

fn lookup_key(env: Option<String>, settings: Option<&Path>, port: Option<u16>) -> Result<String, String> {
    if let Some(k) = env.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        return Ok(k);
    }
    let (Some(settings), Some(port)) = (settings, port) else {
        return Err("no control key: set BRAILLECRAFT_CONTROL_KEY (no settings folder or port to find the key file)".to_string());
    };
    let path = key_path(settings, port);
    match std::fs::read_to_string(&path).map(|k| k.trim().to_string()) {
        Ok(k) if !k.is_empty() => Ok(k),
        _ => Err(format!("no control key: set BRAILLECRAFT_CONTROL_KEY, or start the program with --control {port} (it writes {})", path.display())),
    }
}

fn find_key(addr: &str) -> Result<String, String> {
    let port = addr.rsplit_once(':').and_then(|(_, p)| p.parse().ok());
    lookup_key(std::env::var("BRAILLECRAFT_CONTROL_KEY").ok(), bc_core::paths::settings_dir().as_deref(), port)
}

/// Whether `addr` names a loopback address (127.0.0.0/8, ::1 or localhost); other host names never count.
fn is_loopback_addr(addr: &str) -> bool {
    let loopback = |ip: IpAddr| ip.to_canonical().is_loopback();
    if let Ok(sa) = addr.parse::<SocketAddr>() {
        return loopback(sa.ip());
    }
    if let Ok(ip) = addr.parse::<IpAddr>() {
        return loopback(ip);
    }
    let host = addr.rsplit_once(':').map_or(addr, |(h, _)| h);
    let host = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    host.parse::<IpAddr>().map_or_else(|_| host.eq_ignore_ascii_case("localhost"), loopback)
}

fn may_send_key(addr: &str, peer: Option<SocketAddr>) -> bool {
    is_loopback_addr(addr) && peer.is_some_and(|p| p.ip().to_canonical().is_loopback())
}

impl Remote {
    /// Connects to `addr` (127.0.0.1:7991), failing at once when nothing listens.
    pub fn connect(addr: &str) -> std::io::Result<Self> {
        Self::connect_with(addr, Box::new(find_key))
    }

    fn connect_with(addr: &str, find_key: KeyFinder) -> std::io::Result<Self> {
        let mut r = Self { addr: addr.to_string(), conn: None, next_id: 1, send_key: false, key: None, find_key };
        r.reconnect()?;
        if let Some(w) = r.key_warning() {
            eprintln!("braillecraft-cli mcp: {w}");
        }
        Ok(r)
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Why requests go without the control key, when they do.
    pub fn key_warning(&self) -> Option<String> {
        (!self.send_key).then(|| {
            format!(
                "{} is not a loopback address, so the control key is not sent (it only goes to 127.0.0.0/8, ::1 and localhost) and the program refuses requests; reach the program through 127.0.0.1",
                self.addr
            )
        })
    }

    fn reconnect(&mut self) -> std::io::Result<()> {
        self.conn = None;
        self.key = None;
        let mut last = std::io::Error::new(std::io::ErrorKind::NotFound, format!("cannot resolve {}", self.addr));
        for sa in self.addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&sa, Duration::from_millis(800)) {
                Ok(s) => {
                    s.set_nodelay(true).ok();
                    s.set_read_timeout(Some(Duration::from_secs(120))).ok();
                    self.send_key = may_send_key(&self.addr, s.peer_addr().ok());
                    let read = s.try_clone()?;
                    self.conn = Some((BufReader::new(read), s));
                    return Ok(());
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn request_line(&mut self, id: u64, method: &str, params: &Value) -> Result<String, String> {
        let mut req = json!({"id": id, "method": method, "params": params});
        if self.send_key {
            let key = match &self.key {
                Some(k) => k.clone(),
                None => (self.find_key)(&self.addr)?,
            };
            req["key"] = json!(key);
            self.key = Some(key);
        }
        Ok(req.to_string())
    }

    fn roundtrip(&mut self, line: &str) -> std::io::Result<String> {
        let Some((reader, writer)) = self.conn.as_mut() else {
            return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "not connected"));
        };
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        let mut reply = String::new();
        if reader.read_line(&mut reply)? == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "control channel closed"));
        }
        Ok(reply)
    }

    fn exchange(&mut self, id: u64, method: &str, params: &Value) -> Result<Value, String> {
        let unreachable = |addr: &str, e: std::io::Error| format!("the BrailleCraft program at {addr} is not reachable: {e}");
        let mut first = true;
        let reply = loop {
            let tried = if self.conn.is_some() { Ok(()) } else { self.reconnect() };
            let result = match tried {
                Ok(()) => {
                    let line = self.request_line(id, method, params)?;
                    self.roundtrip(&line)
                }
                Err(e) => Err(e),
            };
            match result {
                Ok(r) => break r,
                Err(e) => {
                    self.conn = None;
                    if !first {
                        return Err(unreachable(&self.addr, e));
                    }
                    first = false;
                }
            }
        };
        serde_json::from_str(reply.trim()).map_err(|e| format!("bad reply from the program: {e}"))
    }
}

impl Backend for Remote {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let unauthorized = |v: &Value| v.get("error").and_then(Value::as_str) == Some("unauthorized");
        let mut v = self.exchange(id, method, &params)?;
        if unauthorized(&v) && self.send_key {
            // A stale key (the program restarted on this port): a new connection reads the key again, once.
            self.conn = None;
            v = self.exchange(id, method, &params)?;
        }
        if v.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(v.get("result").cloned().unwrap_or(Value::Null))
        } else if unauthorized(&v) {
            self.conn = None;
            Err(self.key_warning().map_or_else(
                || format!("unauthorized: the BrailleCraft program at {} refused the control key (from BRAILLECRAFT_CONTROL_KEY or the program's key file)", self.addr),
                |w| format!("unauthorized: {w}"),
            ))
        } else {
            Err(v.get("error").and_then(Value::as_str).unwrap_or("unknown error").to_string())
        }
    }

    fn has_ui(&self) -> bool {
        true
    }

    fn describe(&self) -> String {
        format!("connected to the BrailleCraft program at {}", self.addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_from_the_environment_then_the_key_file() {
        let dir = std::env::temp_dir().join(format!("braillecraft-mcp-key-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = key_path(&dir, 7991);
        let e = lookup_key(None, Some(&dir), Some(7991)).unwrap_err();
        assert!(e.contains("BRAILLECRAFT_CONTROL_KEY") && e.contains(&path.display().to_string()), "{e}");
        std::fs::write(&path, "  file-key\n").unwrap();
        assert_eq!(lookup_key(None, Some(&dir), Some(7991)).unwrap(), "file-key");
        assert_eq!(lookup_key(Some(" env-key ".into()), Some(&dir), Some(7991)).unwrap(), "env-key");
        assert!(lookup_key(None, Some(&dir), Some(7992)).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_key_goes_only_to_loopback_addresses() {
        let peer = |s: &str| s.parse::<SocketAddr>().ok();
        assert!(may_send_key("127.0.0.1:7991", peer("127.0.0.1:7991")));
        assert!(may_send_key("localhost:7991", peer("[::1]:7991")));
        assert!(!may_send_key("192.0.2.20:7991", peer("192.0.2.20:7991")));
        assert!(!may_send_key("localhost:7991", peer("192.0.2.20:7991")));
        assert!(!may_send_key("127.0.0.1:7991", None));
    }

    #[test]
    fn base64_round_trip() {
        for s in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar", &[0u8, 255, 12, 0x0c, b'\n']] {
            assert_eq!(crate::unbase64(&crate::base64(s)).unwrap(), s.to_vec());
        }
        assert_eq!(crate::base64(b"foobar"), "Zm9vYmFy");
    }
}
