//! Every route of the commissions router that names a commission id, answered to someone
//! with no role on it, byte for byte as it answers a made-up id: the reads, the board card's
//! placement and every other call, over the stranger's targets, with a lifecycle word no
//! parser reads and with each time of the row set to infinity; the card of an archived
//! commission; and the Den's grid of what each role is shown at each level, on PostgreSQL and
//! on the in-memory stores. Requires a container runtime socket.

#![allow(clippy::disallowed_methods, reason = "test seeding")]

use adapter_mem::{
    CommissionSeed, MemBackend,
    fixture::{alice, bob, seed_identity},
};
use chrono::Utc;
use domain::elements::{
    commission::{CommissionId, CommissionTitle},
    level::Level,
    profile::Profile,
    user::UserId,
    workflow::ColumnId,
};
use serde_json::Value;
use strum::VariantArray as _;
use test_support::{
    den::{Answer, get},
    http::{client, serve, sign_in},
    pg_app::{
        self, CommissionCall, CommissionTime, DenWorld, Infinity, SeededCommission,
        absent_commission, every_commission_call, placement_call, stranger_targets,
    },
};
use uuid::Uuid;

/// Asserts `call` on `target` answers the stranger exactly as it answers a made-up id: the
/// same status, headers (every one but `date`, which differs per response) and body bytes,
/// and that answer the commission-not-found problem.
async fn assert_answers_like_a_made_up_id(
    world: &DenWorld,
    call: &CommissionCall,
    target: &SeededCommission,
    context: &str,
) {
    let pair = call.pair(target);
    let base = world.base_url();

    let not_yours = call.send(&world.stranger, base, &pair.not_yours).await;
    let absent = call.send(&world.stranger, base, &pair.absent).await;

    let not_yours = Answer::read(not_yours).await;
    let absent = Answer::read(absent).await;
    assert_eq!(not_yours, absent, "{context}, {}", call.label);
    assert_eq!(absent.status, 404, "{context}, {}", call.label);
    let problem: Value = serde_json::from_slice(&absent.body).expect("a JSON body");
    assert_eq!(
        problem["code"], "commission_not_found",
        "{context}, {}",
        call.label
    );
}

/// Asserts every call of the table answers the stranger as a made-up id over each of
/// [`stranger_targets`], the placement into the stranger's own `column` included.
async fn assert_every_call_answers_alike(world: &DenWorld, column: ColumnId, label: &str) {
    let table = every_commission_call(column);
    for (target, visibility) in stranger_targets(world) {
        world.set_visibility(target.id, visibility).await;
        let lifecycle = world.archived_or_live(&target);
        let context = format!("{label}: {visibility} {lifecycle}");
        for call in &table {
            assert_answers_like_a_made_up_id(world, call, &target, &context).await;
        }
    }
}

#[tokio::test]
async fn every_commission_call_answers_a_stranger_as_a_made_up_id() {
    let world = pg_app::den_world(api::app).await;
    let column = world.seed_stranger_board().await.column;

    assert_every_call_answers_alike(&world, column, "plain").await;
}

#[tokio::test]
async fn a_stranger_s_card_of_an_archived_commission_answers_like_a_made_up_id() {
    let world = pg_app::den_world(api::app).await;
    let column = world.seed_stranger_board().await.column;
    let placement = placement_call(column);

    for (target, visibility) in stranger_targets(&world) {
        world.set_visibility(target.id, visibility).await;
        let lifecycle = world.archived_or_live(&target);

        let context = format!("{visibility} {lifecycle}");
        assert_answers_like_a_made_up_id(&world, &placement, &target, &context).await;
    }
}

#[tokio::test]
async fn a_grant_holder_still_places_an_archived_commission() {
    let world = pg_app::den_world(api::app).await;
    let board = world.seed_stranger_board().await;
    let old_sketch = world.old_sketch;
    let grant = serde_json::json!({
        "target_user_id": pg_app::stranger().to_string(),
        "level": "presentation",
    });
    let granting = world
        .alice
        .post(format!(
            "{}/commissions/{}/grants",
            world.base_url(),
            old_sketch.id
        ))
        .json(&grant)
        .send()
        .await
        .expect("the grant is answered");
    assert_eq!(granting.status(), 204, "alice grants the stranger a view");

    // A column takes a commission once, so each level places into a column of its own.
    for visibility in [Level::Listed, Level::Public] {
        world.set_visibility(old_sketch.id, visibility).await;
        let column = world
            .add_stranger_column(&board, &visibility.to_string())
            .await;
        let placement = placement_call(column);

        let path = placement.path_on(&old_sketch);
        let placed = placement
            .send(&world.stranger, world.base_url(), &path)
            .await;

        assert_eq!(placed.status(), 204, "archived, {visibility}");
    }
}

#[tokio::test]
async fn a_stranger_places_a_live_listed_or_public_commission() {
    let world = pg_app::den_world(api::app).await;
    let board = world.seed_stranger_board().await;
    let untitled = world.untitled;

    // A column takes a commission once, so each level places into a column of its own.
    for visibility in [Level::Listed, Level::Public] {
        world.set_visibility(untitled.id, visibility).await;
        let column = world
            .add_stranger_column(&board, &visibility.to_string())
            .await;
        let placement = placement_call(column);

        let path = placement.path_on(&untitled);
        let placed = placement
            .send(&world.stranger, world.base_url(), &path)
            .await;

        assert_eq!(placed.status(), 204, "live, {visibility}");
    }
}

#[tokio::test]
async fn with_a_lifecycle_no_parser_reads_every_call_answers_a_stranger_alike() {
    let world = pg_app::den_world(api::app).await;
    let column = world.seed_stranger_board().await.column;
    world.store_unparsable_lifecycle(world.untitled.id).await;
    world.store_unparsable_lifecycle(world.old_sketch.id).await;

    assert_every_call_answers_alike(&world, column, "an unreadable lifecycle").await;
}

#[tokio::test]
async fn with_infinite_times_every_call_answers_a_stranger_alike() {
    for &time in CommissionTime::VARIANTS {
        // A world each: a planted time stays, and the next column's must not hide behind it.
        let world = pg_app::den_world(api::app).await;
        let column = world.seed_stranger_board().await.column;
        for &infinity in Infinity::VARIANTS {
            for target in [world.untitled, world.old_sketch] {
                world.plant_infinite_time(target.id, time, infinity).await;
            }
            let label = format!("{time} set to {infinity}");

            assert_every_call_answers_alike(&world, column, &label).await;
        }
    }
}

/// Who asks for a commission's node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Viewer {
    /// alice, who owns every commission the grid reads.
    Owner,
    /// bob, a Participant of every one.
    Participant,
    /// The stranger, who has no role on any.
    Stranger,
}

/// The level a commission's node is stored at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stored {
    Private,
    Listed,
    Public,
    /// A word no parser reads, which reads as Private.
    Unreadable,
}

/// Whether the commission is archived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Live,
    Archived,
}

/// What the Den shows at a commission's node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    /// The node, opened.
    Open,
    /// The card of the node: its name and its type, nothing more.
    Card,
    /// The one not-found.
    NotFound,
}

use Lifecycle::{Archived, Live};
use Shown::{Card, NotFound, Open};
use Stored::{Listed, Private, Public, Unreadable};
use Viewer::{Owner, Participant, Stranger};

/// What the Den's access rule shows each role at each level, live or archived: someone with a
/// role opens the node at every level; someone with none opens a live Public one, sees the
/// card of a live Listed one, and finds nothing of a live Private one or of any archived one.
/// A level no parser reads counts as Private.
const GRID: [(Viewer, Stored, Lifecycle, Shown); 24] = [
    (Owner, Private, Live, Open),
    (Owner, Private, Archived, Open),
    (Owner, Listed, Live, Open),
    (Owner, Listed, Archived, Open),
    (Owner, Public, Live, Open),
    (Owner, Public, Archived, Open),
    (Owner, Unreadable, Live, Open),
    (Owner, Unreadable, Archived, Open),
    (Participant, Private, Live, Open),
    (Participant, Private, Archived, Open),
    (Participant, Listed, Live, Open),
    (Participant, Listed, Archived, Open),
    (Participant, Public, Live, Open),
    (Participant, Public, Archived, Open),
    (Participant, Unreadable, Live, Open),
    (Participant, Unreadable, Archived, Open),
    (Stranger, Private, Live, NotFound),
    (Stranger, Private, Archived, NotFound),
    (Stranger, Listed, Live, Card),
    (Stranger, Listed, Archived, NotFound),
    (Stranger, Public, Live, Open),
    (Stranger, Public, Archived, NotFound),
    (Stranger, Unreadable, Live, NotFound),
    (Stranger, Unreadable, Archived, NotFound),
];

/// What `GET /den/commissions/{id}` shows each viewer whatever the level: the route reaches a
/// commission through the viewer's own `commissions` folder, which mounts the commissions
/// they take part in and no others, so a card or an open node needs a role.
const DEN_ROUTE: [(Viewer, Shown); 3] = [(Owner, Open), (Participant, Open), (Stranger, NotFound)];

/// What a `GET /den/commissions/{id}` answer shows.
async fn shown_by_the_den_route(response: reqwest::Response) -> Shown {
    if response.status() == 404 {
        return NotFound;
    }
    assert_eq!(response.status(), 200, "{}", response.url());
    let body: Value = response.json().await.expect("a JSON body");
    let node = &body["node"];
    match (node.get("open"), node.get("card")) {
        (Some(_), None) => Open,
        (None, Some(_)) => Card,
        _ => panic!("a node is open or a card: {node}"),
    }
}

/// What a `GET /commissions/{id}` answer shows. The read is behind the Den's access check,
/// which refuses a card as it refuses nothing, so a card reads as the not-found.
async fn shown_by_the_read(response: reqwest::Response) -> Shown {
    match response.status().as_u16() {
        200 => Open,
        404 => NotFound,
        other => panic!("a read answers 200 or 404, not {other}: {}", response.url()),
    }
}

/// What the read behind the Den's check shows where the Den's rule shows `shown`.
fn as_the_read_shows(shown: Shown) -> Shown {
    match shown {
        Open => Open,
        Card | NotFound => NotFound,
    }
}

#[tokio::test]
async fn the_summary_hides_exactly_what_the_den_refuses_on_postgres() {
    let world = pg_app::den_world(api::app).await;
    world.add_bob_to_old_sketch().await;

    for (viewer, stored, lifecycle, expected) in GRID {
        let target = match lifecycle {
            Live => world.untitled,
            Archived => world.old_sketch,
        };
        match stored {
            Private => world.set_visibility(target.id, Level::Private).await,
            Listed => world.set_visibility(target.id, Level::Listed).await,
            Public => world.set_visibility(target.id, Level::Public).await,
            Unreadable => world.store_unparsable_level(target.id).await,
        }
        let client = match viewer {
            Owner => &world.alice,
            Participant => &world.bob,
            Stranger => &world.stranger,
        };
        let read_path = format!("/commissions/{}", target.id);

        let read = shown_by_the_read(get(client, world.base_url(), &read_path).await).await;

        let context = format!("{viewer:?}, {stored:?}, {lifecycle:?}, on PostgreSQL");
        assert_eq!(read, as_the_read_shows(expected), "{context}: the read");
    }
}

#[tokio::test]
async fn the_den_route_shows_a_commission_to_its_roles_alone_on_postgres() {
    let world = pg_app::den_world(api::app).await;
    world.add_bob_to_old_sketch().await;

    for (viewer, stored, lifecycle, _) in GRID {
        let target = match lifecycle {
            Live => world.untitled,
            Archived => world.old_sketch,
        };
        match stored {
            Private => world.set_visibility(target.id, Level::Private).await,
            Listed => world.set_visibility(target.id, Level::Listed).await,
            Public => world.set_visibility(target.id, Level::Public).await,
            Unreadable => world.store_unparsable_level(target.id).await,
        }
        let client = match viewer {
            Owner => &world.alice,
            Participant => &world.bob,
            Stranger => &world.stranger,
        };
        let den_path = format!("/den/commissions/{}", target.id);

        let den = shown_by_the_den_route(get(client, world.base_url(), &den_path).await).await;

        let context = format!("{viewer:?}, {stored:?}, {lifecycle:?}, on PostgreSQL");
        let (_, expected) = DEN_ROUTE
            .into_iter()
            .find(|(role, _)| *role == viewer)
            .expect("the route table names every viewer");
        assert_eq!(den, expected, "{context}: the Den route");
    }
}

/// The id of the grid's commission at `stored` and `lifecycle` on the in-memory stores.
fn grid_commission(stored: Stored, lifecycle: Lifecycle) -> CommissionId {
    let level_part: u128 = match stored {
        Private => 1,
        Listed => 2,
        Public => 3,
        Unreadable => 4,
    };
    let lifecycle_part: u128 = match lifecycle {
        Live => 1,
        Archived => 2,
    };
    let word = 0x0193_0000_0000_7000_8000_0000_0000_0000_u128 + level_part * 16 + lifecycle_part;
    CommissionId::from(Uuid::from_u128(word))
}

/// Seeds one commission of alice's, bob its Participant, for each readable level, live and
/// archived. The in-memory stores hold a level only as the vocabulary names it, so there is
/// none for a word no parser reads, and none for a commission with no node, which they refuse
/// to hold at all.
async fn seed_grid(backend: &MemBackend) {
    for stored in [Private, Listed, Public] {
        for lifecycle in [Live, Archived] {
            let level = match stored {
                Private => Level::Private,
                Listed => Level::Listed,
                Public | Unreadable => Level::Public,
            };
            let id = grid_commission(stored, lifecycle);
            let title = "A grid row"
                .parse::<CommissionTitle>()
                .expect("a valid title");
            let seed = CommissionSeed {
                id,
                title,
                level,
                owner_id: alice(),
                created_at: Utc::now(),
                deadline: None,
                maturity: None,
            };
            backend
                .seed_bound_commission(&seed)
                .await
                .expect("seeds a grid commission");
            backend.seed_participant(id, bob(), Utc::now());
            if lifecycle == Archived {
                let database = backend.database();
                let mut unit = database.begin().await.expect("begins a unit");
                unit.commissions()
                    .set_archived(&id, Some(Utc::now()))
                    .await
                    .expect("archives a grid commission");
                unit.commit().await.expect("commits the archive");
            }
        }
    }
}

/// A served in-memory world with the grid seeded and `viewer` signed in as `handle`.
async fn mem_world_as(viewer: &UserId, handle: &str) -> (String, reqwest::Client) {
    let profile = Profile::new(viewer.did().clone(), handle);
    let builder = test_support::runtime::mem(viewer.did()).profile(profile);
    let served = serve(builder, api::app).await;
    seed_identity(&served.backend)
        .await
        .expect("seeds identity's fixtures");
    seed_grid(&served.backend).await;
    let signed_in = client();
    sign_in(&signed_in, &served.base_url, handle).await;
    (served.base_url, signed_in)
}

/// The three viewers' served in-memory worlds.
struct MemWorlds {
    owner: (String, reqwest::Client),
    participant: (String, reqwest::Client),
    stranger: (String, reqwest::Client),
}

impl MemWorlds {
    async fn serve() -> Self {
        Self {
            owner: mem_world_as(&alice(), "alice.test").await,
            participant: mem_world_as(&bob(), "bob.test").await,
            stranger: mem_world_as(&pg_app::stranger(), "stranger.test").await,
        }
    }

    /// The base URL and the client of `viewer`'s world.
    fn of(&self, viewer: Viewer) -> &(String, reqwest::Client) {
        match viewer {
            Owner => &self.owner,
            Participant => &self.participant,
            Stranger => &self.stranger,
        }
    }
}

#[tokio::test]
async fn the_summary_hides_exactly_what_the_den_refuses_on_the_in_memory_stores() {
    let worlds = MemWorlds::serve().await;

    for (viewer, stored, lifecycle, expected) in GRID {
        if stored == Unreadable {
            continue;
        }
        let (base, client) = worlds.of(viewer);
        let commission = grid_commission(stored, lifecycle);
        let read_path = format!("/commissions/{commission}");

        let read = shown_by_the_read(get(client, base, &read_path).await).await;

        let context = format!("{viewer:?}, {stored:?}, {lifecycle:?}, in memory");
        assert_eq!(read, as_the_read_shows(expected), "{context}: the read");
    }
}

#[tokio::test]
async fn the_den_route_shows_a_commission_to_its_roles_alone_in_memory() {
    let worlds = MemWorlds::serve().await;

    for (viewer, stored, lifecycle, _) in GRID {
        if stored == Unreadable {
            continue;
        }
        let (base, client) = worlds.of(viewer);
        let commission = grid_commission(stored, lifecycle);
        let den_path = format!("/den/commissions/{commission}");

        let den = shown_by_the_den_route(get(client, base, &den_path).await).await;

        let context = format!("{viewer:?}, {stored:?}, {lifecycle:?}, in memory");
        let (_, expected) = DEN_ROUTE
            .into_iter()
            .find(|(role, _)| *role == viewer)
            .expect("the route table names every viewer");
        assert_eq!(den, expected, "{context}: the Den route");
    }
}

#[tokio::test]
async fn a_made_up_commission_is_not_found_for_everyone_in_memory() {
    let worlds = MemWorlds::serve().await;
    let read_path = format!("/commissions/{}", absent_commission());
    let den_path = format!("/den/commissions/{}", absent_commission());

    for viewer in [Owner, Participant, Stranger] {
        let (base, client) = worlds.of(viewer);

        let read = shown_by_the_read(get(client, base, &read_path).await).await;
        let den = shown_by_the_den_route(get(client, base, &den_path).await).await;

        assert_eq!(read, NotFound, "{viewer:?}: the read");
        assert_eq!(den, NotFound, "{viewer:?}: the Den route");
    }
}
