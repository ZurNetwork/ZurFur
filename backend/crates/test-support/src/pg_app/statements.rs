//! A recording proxy between the app and PostgreSQL: it passes every byte through unchanged and
//! logs each statement the app asks PostgreSQL to execute, read off the frontend protocol, so
//! a test reads what the database ran rather than which port method was called.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};

/// The startup-phase request codes that precede the real startup message.
const SSL_REQUEST: i32 = 80_877_103;
const GSS_ENCRYPTION_REQUEST: i32 = 80_877_104;

/// One statement PostgreSQL was asked to execute: its text and its bound parameter values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    /// The SQL text, as sent.
    pub sql: String,
    /// Each bound parameter's bytes, `None` for a NULL; empty for a simple query.
    pub parameters: Vec<Option<Vec<u8>>>,
}

impl Statement {
    /// The adapter's query file this text was read from, as `<dir>/<name>`, if it is one.
    pub fn query_name(&self) -> Option<&'static str> {
        query_names().get(self.sql.as_str()).map(String::as_str)
    }
}

/// The query file's name, else the SQL on one line.
impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = self.query_name() {
            return f.write_str(name);
        }
        let one_line = self.sql.split_whitespace().collect::<Vec<_>>().join(" ");
        f.write_str(&one_line)
    }
}

/// Every statement the proxied app executed, in the order it was sent. Clones share one log.
#[derive(Debug, Clone, Default)]
pub struct StatementLog {
    statements: Arc<Mutex<Vec<Statement>>>,
}

impl StatementLog {
    /// How many statements have been executed so far: a mark to read the later ones from.
    pub fn mark(&self) -> usize {
        self.statements.lock().expect("statement log lock").len()
    }

    /// The statements executed after `mark`, in order.
    pub fn since(&self, mark: usize) -> Vec<Statement> {
        let statements = self.statements.lock().expect("statement log lock");
        statements[mark..].to_vec()
    }

    fn push(&self, statement: Statement) {
        self.statements
            .lock()
            .expect("statement log lock")
            .push(statement);
    }
}

/// Starts a proxy in front of the database at `database_url` that logs into `log`; returns
/// the URL that reaches the same database through it, TLS off so the protocol stays legible.
pub async fn record_statements(database_url: &str, log: StatementLog) -> String {
    let mut url = url::Url::parse(database_url).expect("the database URL parses");
    let host = url.host_str().expect("the database URL names a host");
    let port = url.port().unwrap_or(5432);
    let upstream = format!("{host}:{port}");
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the statement proxy");
    let proxy_port = listener.local_addr().expect("the proxy's address").port();
    url.set_host(Some("127.0.0.1"))
        .expect("the database URL takes a host");
    url.set_port(Some(proxy_port))
        .expect("the database URL takes a port");
    url.query_pairs_mut().append_pair("sslmode", "disable");
    tokio::spawn(accept(listener, upstream, log));
    url.to_string()
}

/// Relays every connection the app opens to `upstream`.
async fn accept(listener: TcpListener, upstream: String, log: StatementLog) {
    while let Ok((client, _)) = listener.accept().await {
        tokio::spawn(relay(client, upstream.clone(), log.clone()));
    }
}

/// Passes `client`'s bytes to `upstream` and back, logging each statement on the way in
/// before it is forwarded, so it is in the log before PostgreSQL can answer it.
async fn relay(client: TcpStream, upstream: String, log: StatementLog) {
    let Ok(server) = TcpStream::connect(&upstream).await else {
        return;
    };
    let _ = client.set_nodelay(true);
    let _ = server.set_nodelay(true);
    let (mut client_read, mut client_write) = client.into_split();
    let (mut server_read, mut server_write) = server.into_split();
    let answers = tokio::io::copy(&mut server_read, &mut client_write);
    let requests = async {
        let mut reader = FrontendReader::default();
        let mut chunk = vec![0_u8; 16 * 1024];
        loop {
            let read = client_read.read(&mut chunk).await?;
            if read == 0 {
                break;
            }
            for statement in reader.feed(&chunk[..read]) {
                log.push(statement);
            }
            server_write.write_all(&chunk[..read]).await?;
        }
        server_write.shutdown().await
    };
    let _ = tokio::join!(answers, requests);
}

/// Reads one connection's frontend messages as they arrive, in any chunking, and yields each
/// statement executed: a simple query, or an extended-protocol `Execute` of a bound portal.
/// Only the frontend side is read; anything it can't parse is passed over.
#[derive(Debug, Default)]
struct FrontendReader {
    /// Past the startup message, so every message carries a type byte.
    started: bool,
    /// Bytes not yet making up a whole message.
    pending: Vec<u8>,
    /// The SQL of each prepared statement, by name (`""` is the unnamed one).
    prepared: HashMap<String, String>,
    /// The statement each portal was bound to, with its parameters, by name.
    bound: HashMap<String, Statement>,
}

impl FrontendReader {
    /// Takes in `bytes` and returns the statements executed by the messages now whole.
    fn feed(&mut self, bytes: &[u8]) -> Vec<Statement> {
        self.pending.extend_from_slice(bytes);
        let mut executed = Vec::new();
        loop {
            let header = if self.started { 5 } else { 4 };
            if self.pending.len() < header {
                break;
            }
            let length_at = header - 4;
            let length = read_i32(&self.pending[length_at..header]);
            let Ok(length) = usize::try_from(length) else {
                break;
            };
            let total = length_at + length;
            if length < 4 || self.pending.len() < total {
                break;
            }
            let message: Vec<u8> = self.pending.drain(..total).collect();
            if self.started {
                let kind = message[0];
                executed.extend(self.read_message(kind, &message[header..]));
            } else {
                self.read_startup(&message[header..]);
            }
        }
        executed
    }

    /// A startup-phase message: an encryption request keeps the startup phase open, anything
    /// else ends it.
    fn read_startup(&mut self, body: &[u8]) {
        let code = body.get(..4).map(read_i32);
        let asks_for_encryption = matches!(code, Some(SSL_REQUEST | GSS_ENCRYPTION_REQUEST));
        self.started = !asks_for_encryption;
    }

    /// One typed message: the statement it executes, if it executes one.
    fn read_message(&mut self, kind: u8, body: &[u8]) -> Option<Statement> {
        let mut message = Fields::new(body);
        match kind {
            b'Q' => {
                let sql = message.text()?;
                let simple_query = Statement {
                    sql,
                    parameters: Vec::new(),
                };
                Some(simple_query)
            }
            b'P' => {
                let name = message.text()?;
                let sql = message.text()?;
                self.prepared.insert(name, sql);
                None
            }
            b'B' => {
                let portal = message.text()?;
                let name = message.text()?;
                let format_count = message.count()?;
                message.skip(2 * format_count)?;
                let parameter_count = message.count()?;
                let parameters = (0..parameter_count)
                    .map(|_| message.value())
                    .collect::<Option<Vec<_>>>()?;
                let sql = self.prepared.get(&name)?.clone();
                self.bound.insert(portal, Statement { sql, parameters });
                None
            }
            b'E' => {
                let portal = message.text()?;
                self.bound.get(&portal).cloned()
            }
            b'C' => {
                let closes_a_statement = message.byte()? == b'S';
                let name = message.text()?;
                if closes_a_statement {
                    self.prepared.remove(&name);
                } else {
                    self.bound.remove(&name);
                }
                None
            }
            _ => None,
        }
    }
}

/// A big-endian `i32` from exactly four bytes.
fn read_i32(bytes: &[u8]) -> i32 {
    let bytes: [u8; 4] = bytes.try_into().expect("four bytes");
    i32::from_be_bytes(bytes)
}

/// The fields of one message's body, read front to back.
struct Fields<'a> {
    rest: &'a [u8],
}

impl<'a> Fields<'a> {
    fn new(body: &'a [u8]) -> Self {
        Self { rest: body }
    }

    /// The next `n` bytes.
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (taken, rest) = self.rest.split_at_checked(n)?;
        self.rest = rest;
        Some(taken)
    }

    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(drop)
    }

    fn byte(&mut self) -> Option<u8> {
        self.take(1).map(|taken| taken[0])
    }

    /// A NUL-terminated string.
    fn text(&mut self) -> Option<String> {
        let end = self.rest.iter().position(|byte| *byte == 0)?;
        let text = self.take(end)?;
        self.skip(1)?;
        String::from_utf8(text.to_vec()).ok()
    }

    /// A non-negative `i16` count.
    fn count(&mut self) -> Option<usize> {
        let bytes: [u8; 2] = self.take(2)?.try_into().ok()?;
        usize::try_from(i16::from_be_bytes(bytes)).ok()
    }

    /// A length-prefixed parameter value, `None` inside for a NULL (length -1).
    fn value(&mut self) -> Option<Option<Vec<u8>>> {
        let length = read_i32(self.take(4)?);
        if length < 0 {
            return Some(None);
        }
        let length = usize::try_from(length).ok()?;
        self.take(length).map(|bytes| Some(bytes.to_vec()))
    }
}

/// Every query file of the PostgreSQL adapter, by its exact text, named `<dir>/<name>`.
fn query_names() -> &'static HashMap<String, String> {
    static NAMES: OnceLock<HashMap<String, String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let queries = Path::new(env!("CARGO_MANIFEST_DIR")).join("../adapter-pg/queries");
        let mut names = HashMap::new();
        let directories = std::fs::read_dir(&queries).expect("the adapter's queries directory");
        for directory in directories.flatten() {
            let Some(dir) = directory.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(files) = std::fs::read_dir(directory.path()) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                let Ok(sql) = std::fs::read_to_string(&path) else {
                    continue;
                };
                names.insert(sql, format!("{dir}/{stem}"));
            }
        }
        names
    })
}

#[cfg(test)]
mod tests;
