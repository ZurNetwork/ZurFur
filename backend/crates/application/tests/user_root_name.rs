//! The viewer's root named from their own PDS through the den Query over the in-memory
//! runtime: the cache first, one lookup of the viewer's own DID that only the first read
//! waits on, the DID whenever the PDS is slow, down or busy.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use application::{
    den::{
        DenError,
        node::{self, NodeQuery},
        root::{self, RootQuery},
    },
    user::{FAILED_LOOKUP_HOLD, PROFILE_WAIT},
};
use async_trait::async_trait;
use domain::{
    elements::{did::Did, profile::Profile, user::UserId},
    ports::ProfileSource,
};
use tokio::time::Instant;

/// How the PDS answers a fetch.
#[derive(Clone)]
enum Answer {
    /// Never answers.
    Never,
    /// Fails at once.
    Fails,
    /// Answers with the profile after the delay.
    After(Duration, Profile),
}

/// A PDS that answers as told and records every DID it is asked for.
struct ScriptedPds {
    answer: Answer,
    asked_for: Mutex<Vec<Did>>,
}

impl ScriptedPds {
    fn answering(answer: Answer) -> Arc<Self> {
        let pds = Self {
            answer,
            asked_for: Mutex::new(Vec::new()),
        };
        Arc::new(pds)
    }

    fn asked_for(&self) -> Vec<Did> {
        self.asked_for.lock().expect("the record's lock").clone()
    }
}

#[async_trait]
impl ProfileSource for ScriptedPds {
    async fn fetch(&self, did: &Did) -> anyhow::Result<Profile> {
        self.asked_for
            .lock()
            .expect("the record's lock")
            .push(did.clone());
        match self.answer.clone() {
            Answer::Never => std::future::pending().await,
            Answer::Fails => Err(anyhow::anyhow!("the PDS refused (test)")),
            Answer::After(delay, profile) => {
                tokio::time::sleep(delay).await;
                Ok(profile)
            }
        }
    }
}

fn carol() -> Did {
    Did::from("did:plc:root-name-carol".to_owned())
}

/// The in-memory runtime with carol signed up, her profile uncached, behind `pds`.
async fn stack(pds: &Arc<ScriptedPds>) -> composition::Runtime {
    let mut mem = test_support::runtime::mem(&carol()).build();
    mem.backend
        .provision(&carol())
        .await
        .expect("recognizes carol");
    mem.runtime.profile_source = pds.clone();
    mem.runtime
}

/// Carol's root name, read as one request, and how long the read took.
async fn timed_root(runtime: &composition::Runtime) -> (String, Duration) {
    let query = RootQuery {
        viewer: UserId::from(carol()),
        include_deleted: false,
        page: None,
    };
    let started = Instant::now();
    let output: root::Output = runtime.app().den().root(query).await.expect("carol's root");
    (output.node.name.to_string(), started.elapsed())
}

/// The node at `path` below carol's root, read as one request, and how long the read took.
async fn timed_read(
    runtime: &composition::Runtime,
    path: &[&str],
) -> (Result<node::Output, DenError>, Duration) {
    let query = NodeQuery {
        viewer: UserId::from(carol()),
        path: path.iter().map(|piece| (*piece).to_owned()).collect(),
        include_deleted: false,
        page: None,
    };
    let started = Instant::now();
    let output = runtime.app().den().node(query).await;
    (output, started.elapsed())
}

fn carol_s_profile() -> Profile {
    Profile::new(carol(), "carol.test").with_display_name("Carol")
}

#[tokio::test(start_paused = true)]
async fn a_pds_that_never_answers_names_the_root_by_the_did_within_the_wait() {
    let pds = ScriptedPds::answering(Answer::Never);
    let runtime = stack(&pds).await;

    let (name, waited) = timed_root(&runtime).await;

    assert_eq!(name, carol().to_string());
    assert_eq!(waited, PROFILE_WAIT);
}

#[tokio::test(start_paused = true)]
async fn a_deep_page_against_a_silent_pds_finishes_within_one_wait_and_only_its_first_read_waits() {
    let pds = ScriptedPds::answering(Answer::Never);
    let runtime = stack(&pds).await;
    let started = Instant::now();

    let (_, root_wait) = timed_root(&runtime).await;
    let mut later_waits = Vec::new();
    for path in [["accounts"], ["characters"], ["commissions"]] {
        let (read, waited) = timed_read(&runtime, &path).await;
        read.expect("a folder in carol's root opens");
        later_waits.push(waited);
    }
    let (name_again, again_wait) = timed_root(&runtime.clone()).await;
    later_waits.push(again_wait);

    assert_eq!(root_wait, PROFILE_WAIT);
    assert_eq!(later_waits, vec![Duration::ZERO; 4]);
    assert_eq!(started.elapsed(), PROFILE_WAIT);
    assert_eq!(name_again, carol().to_string());
    assert_eq!(pds.asked_for(), [carol()]);
}

#[tokio::test(start_paused = true)]
async fn after_a_failed_lookup_reads_name_the_did_at_once_until_the_hold_ends() {
    let pds = ScriptedPds::answering(Answer::Fails);
    let runtime = stack(&pds).await;
    timed_root(&runtime).await;

    tokio::time::sleep(FAILED_LOOKUP_HOLD - Duration::from_secs(1)).await;
    let (held_name, held_wait) = timed_root(&runtime).await;
    let asked_within_the_hold = pds.asked_for().len();
    tokio::time::sleep(Duration::from_secs(1)).await;
    timed_root(&runtime).await;

    assert_eq!(held_name, carol().to_string());
    assert_eq!(held_wait, Duration::ZERO);
    assert_eq!(asked_within_the_hold, 1);
    assert_eq!(pds.asked_for(), [carol(), carol()]);
}

#[tokio::test(start_paused = true)]
async fn a_slow_pds_names_the_root_by_the_did_first_then_by_the_display_name() {
    let slow = PROFILE_WAIT + Duration::from_secs(3);
    let pds = ScriptedPds::answering(Answer::After(slow, carol_s_profile()));
    let runtime = stack(&pds).await;

    let (first, _) = timed_root(&runtime).await;
    tokio::time::sleep(slow).await;
    let (later, later_wait) = timed_root(&runtime).await;

    assert_eq!(first, carol().to_string());
    assert_eq!(later, "Carol");
    assert_eq!(later_wait, Duration::ZERO);
    assert_eq!(pds.asked_for(), [carol()]);
}

#[tokio::test(start_paused = true)]
async fn the_name_rule_takes_the_display_name_else_the_checked_handle_else_the_did() {
    let profiles = [
        (carol_s_profile(), "Carol".to_owned()),
        (Profile::new(carol(), "carol.test"), "carol.test".to_owned()),
        (Profile::new(carol(), "handle.invalid"), carol().to_string()),
    ];

    for (profile, expected) in profiles {
        let pds = ScriptedPds::answering(Answer::After(Duration::ZERO, profile));
        let runtime = stack(&pds).await;

        let (name, _) = timed_root(&runtime).await;

        assert_eq!(name, expected);
    }
}
