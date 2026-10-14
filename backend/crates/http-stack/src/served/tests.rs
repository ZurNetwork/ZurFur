use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use axum::{
    body::{Body, to_bytes},
    http::StatusCode,
    middleware::{Next, from_fn},
    routing::{get, post},
};
use futures_util::{StreamExt as _, stream};
use tokio::io::AsyncWriteExt as _;
use tokio::sync::Notify;
use tower_sessions::{
    MemoryStore, Session,
    session::{Id, Record},
    session_store,
};

use super::*;

/// How long a test waits for an event that must come.
const WAIT: Duration = Duration::from_secs(60);

type Events = Arc<Mutex<Vec<&'static str>>>;

/// Waits at most [`WAIT`] for `event`, failing with `what` if it never comes.
async fn within<F: Future>(what: &str, event: F) -> F::Output {
    tokio::time::timeout(WAIT, event)
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

/// A session store over [`MemoryStore`] counting the records it writes.
#[derive(Clone, Debug, Default)]
struct CountingStore {
    inner: MemoryStore,
    writes: Arc<Mutex<usize>>,
}

impl CountingStore {
    fn writes(&self) -> usize {
        *self.writes.lock().expect("unpoisoned")
    }

    fn count_write(&self) {
        *self.writes.lock().expect("unpoisoned") += 1;
    }
}

#[async_trait::async_trait]
impl SessionStore for CountingStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        self.inner.create(record).await?;
        self.count_write();
        Ok(())
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        self.inner.save(record).await?;
        self.count_write();
        Ok(())
    }

    async fn load(&self, id: &Id) -> session_store::Result<Option<Record>> {
        self.inner.load(id).await
    }

    async fn delete(&self, id: &Id) -> session_store::Result<()> {
        self.inner.delete(id).await
    }
}

/// Signals `dropped` when dropped while still armed.
struct DropSignal(Option<Arc<Notify>>);

impl DropSignal {
    /// Stops the signal: the request answered.
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for DropSignal {
    fn drop(&mut self) {
        if let Some(dropped) = self.0.take() {
            dropped.notify_one();
        }
    }
}

/// `stacked` under an outer layer that signals `dropped` when a request's
/// future is dropped before it answered.
fn witnessed(stacked: Stacked, dropped: &Arc<Notify>) -> Stacked {
    let dropped = Arc::clone(dropped);
    let witness = from_fn(move |request: Request, next: Next| {
        let mut signal = DropSignal(Some(Arc::clone(&dropped)));
        async move {
            let response = next.run(request).await;
            signal.disarm();
            response
        }
    });
    stacked.layer(witness)
}

/// Serves `stacked` on a loopback port until `shutdown`, recording `exited`
/// into `events` once `serve` returns; its address and task.
async fn served(
    stacked: Stacked,
    shutdown: impl Future<Output = ()> + Send + 'static,
    events: &Events,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let address = listener.local_addr().expect("an address");
    let events = Arc::clone(events);
    let server = tokio::spawn(async move {
        serve(listener, stacked, shutdown).await.expect("serves");
        events.lock().expect("unpoisoned").push("exited");
    });
    (address, server)
}

/// Sends `GET /held` and drops the connection once the handler has `entered`,
/// then waits until the server dropped the request's future.
async fn cut_held(address: std::net::SocketAddr, entered: &Notify, dropped: &Notify) {
    let mut client = tokio::net::TcpStream::connect(address)
        .await
        .expect("connects");
    client
        .write_all(b"GET /held HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .expect("sends");
    within("the handler", entered.notified()).await;
    drop(client);
    within("the request's future dropped", dropped.notified()).await;
}

#[tokio::test]
async fn a_cut_request_still_saves_its_session() {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let handler = {
        let (entered, release) = (entered.clone(), release.clone());
        move |session: Session| async move {
            session.insert("seen", true).await.expect("inserts");
            entered.notify_one();
            release.notified().await;
            StatusCode::NO_CONTENT
        }
    };
    let store = CountingStore::default();
    let app = Router::new().route("/held", get(handler));
    let sessions = http_cookies::session_layer(store.clone(), false);
    let dropped = Arc::new(Notify::new());
    let stacked = witnessed(stack(app, sessions, Detach::default()), &dropped);
    let detach = stacked.detach().clone();
    let (address, _server) = served(stacked, std::future::pending(), &Events::default()).await;

    cut_held(address, &entered, &dropped).await;
    release.notify_one();
    within("the drain", detach.drain(Instant::now() + WAIT))
        .await
        .expect("drained");

    assert_eq!(store.writes(), 1, "the cut request's session was saved");
}

#[tokio::test]
async fn shutdown_holds_while_a_detached_request_runs() {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let events = Events::default();
    let handler = {
        let (entered, release, events) = (entered.clone(), release.clone(), events.clone());
        move || async move {
            entered.notify_one();
            release.notified().await;
            events.lock().expect("unpoisoned").push("finished");
            StatusCode::NO_CONTENT
        }
    };
    let app = Router::new().route("/held", get(handler));
    let sessions = http_cookies::session_layer(MemoryStore::default(), false);
    let dropped = Arc::new(Notify::new());
    let stacked = witnessed(stack(app, sessions, Detach::default()), &dropped);
    let (stop, stopped) = oneshot::channel::<()>();
    let shutdown = async move {
        let _ = stopped.await;
    };
    let (address, mut server) = served(stacked, shutdown, &events).await;

    cut_held(address, &entered, &dropped).await;
    stop.send(()).expect("the server listens for its signal");
    let early = tokio::time::timeout(Duration::from_secs(2), &mut server).await;
    assert!(
        early.is_err(),
        "the server exited with a detached request still running"
    );
    release.notify_one();
    within("the server's exit", server)
        .await
        .expect("the server task ends");

    let order = events.lock().expect("unpoisoned").clone();
    assert_eq!(order, ["finished", "exited"]);
}

#[tokio::test(start_paused = true)]
async fn a_stalled_body_fails_its_read_and_frees_the_permit() {
    let events = Events::default();
    let handler = {
        let events = events.clone();
        move |body: Body| async move {
            let read = to_bytes(body, usize::MAX).await;
            let outcome = if read.is_err() {
                "body error"
            } else {
                "body read"
            };
            events.lock().expect("unpoisoned").push(outcome);
            StatusCode::REQUEST_TIMEOUT
        }
    };
    let app = Router::new()
        .route("/upload", post(handler))
        .route("/quick", get(|| async { StatusCode::NO_CONTENT }));
    let sessions = http_cookies::session_layer(MemoryStore::default(), false);
    let mut router = stack(app, sessions, Detach::new(1)).router;
    let first_frame = stream::iter([Ok::<_, std::io::Error>("the first frame")]);
    let stalled = Body::from_stream(first_frame.chain(stream::pending()));
    let upload = Request::post("/upload")
        .body(stalled)
        .expect("a valid request");
    let quick = Request::get("/quick")
        .body(Body::empty())
        .expect("a valid request");

    let started = Instant::now();
    let Ok::<_, Infallible>(stalled_answer) =
        within("the stalled upload", router.call(upload)).await;
    let waited = started.elapsed();
    let Ok::<_, Infallible>(quick_answer) = within("the next request", router.call(quick)).await;

    assert_eq!(stalled_answer.status(), StatusCode::REQUEST_TIMEOUT);
    assert_eq!(waited, BODY_IDLE_TIMEOUT);
    assert_eq!(*events.lock().expect("unpoisoned"), ["body error"]);
    assert_eq!(quick_answer.status(), StatusCode::NO_CONTENT);
}
