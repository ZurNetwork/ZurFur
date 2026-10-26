//! The commission maturity posture over PostgreSQL, against a throwaway
//! container: the nullable
//! `maturity` + `graphic` envelope column pair starts NULL (a commission is
//! born unrated), `set_maturity` writes both halves on the open unit of work
//! by compare-and-set on the held pair, `find` re-validates them through the
//! domain gates, and the schema's both-or-neither CHECK makes a half-set
//! posture unrepresentable. Requires a container runtime socket (DOCKER_HOST
//! honored).

use crate::seed;
use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use domain::{
    elements::{
        commission::{Commission, CommissionState, MaturitySet, rate_maturity},
        did::Did,
        maturity::{Maturity, MaturityRating},
        user::User,
    },
    ports::{CommissionRows, Database},
};
use strum::VariantArray;

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// Recognize a visitor in its own committed unit of work (`commission.owner_id`
/// references `users(id)`).
async fn provision(pool: &PgPool, did: &str) -> User {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::from(did.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// Create and commit a commission owned by `owner_did`, returning it.
async fn seed_commission(pool: &PgPool, owner_did: &str) -> Commission {
    let owner = provision(pool, owner_did).await;
    seed::seed_stored(pool, &owner.id, "A ref sheet").await
}

/// The rating `to` on what the store holds for `commission`, as its rule checks it.
async fn rating_of(pool: &PgPool, commission: &Commission, to: Maturity) -> MaturitySet {
    let stored = PgCommissionStore::new(pool.clone())
        .find(&commission.id)
        .await
        .expect("find")
        .expect("exists");
    rate_maturity(&CommissionState::from(&stored), to).expect("a real rating")
}

/// Writes `change` in its own committed unit of work, answering whether it landed.
async fn write_rating(pool: &PgPool, commission: &Commission, change: &MaturitySet) -> bool {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let landed = uow
        .commissions()
        .set_maturity(&seed::owner_on(commission.id), change)
        .await
        .expect("set maturity");
    uow.commit().await.expect("commit");
    landed
}

/// Rates `commission` `maturity` in its own committed unit of work; it lands.
async fn set_maturity(pool: &PgPool, commission: &Commission, maturity: Maturity) {
    let change = rating_of(pool, commission, maturity).await;
    assert!(
        write_rating(pool, commission, &change).await,
        "{maturity:?}"
    );
}

// The birth invariant (pg): a freshly created commission reads back unrated —
// both columns NULL, rebuilt as `None`.
#[tokio::test]
async fn a_created_commission_starts_unrated() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:unrated-owner").await;
    let store = PgCommissionStore::new(pool.clone());

    let found = store
        .find(&commission.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(found.maturity, None, "born unrated (ZMVP-31 invariant)");
}

// Every axis value round-trips with either Graphic arm, and a later write
// replaces the posture (replace-only; there is no clear).
#[tokio::test]
async fn set_maturity_round_trips_and_replaces() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:rating-owner").await;
    let store = PgCommissionStore::new(pool.clone());

    for rating in MaturityRating::VARIANTS {
        for graphic in [true, false] {
            let posture = Maturity {
                rating: *rating,
                graphic,
            };
            set_maturity(&pool, &commission, posture).await;
            assert_eq!(
                store
                    .find(&commission.id)
                    .await
                    .expect("find")
                    .expect("exists")
                    .maturity,
                Some(posture),
                "posture {posture:?} round-trips and replaces the previous one",
            );
        }
    }
}

// The write shares its unit of work: a dropped (rolled-back) unit leaves the
// commission unrated.
#[tokio::test]
async fn set_maturity_rolls_back_with_its_unit() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:rollback-owner").await;
    let store = PgCommissionStore::new(pool.clone());

    let adult = Maturity {
        rating: MaturityRating::Adult,
        graphic: true,
    };
    let change = rating_of(&pool, &commission, adult).await;
    {
        let db = PgDatabase::new(pool.clone());
        let mut uow = db.begin().await.expect("begin");
        uow.commissions()
            .set_maturity(&seed::owner_on(commission.id), &change)
            .await
            .expect("set maturity");
        // `uow` drops here without `commit` → the staged write is discarded.
    }

    assert_eq!(
        store
            .find(&commission.id)
            .await
            .expect("find")
            .expect("exists")
            .maturity,
        None,
        "a dropped unit of work rates nothing",
    );
}

// A rating checked from one the row no longer holds matches no row: one checked while it was
// unrated, once it is rated, and one checked from another Graphic flag. Neither lands, and the
// rating held stays.
#[tokio::test]
async fn a_stale_rating_matches_no_row() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:stale-rating-owner").await;
    let store = PgCommissionStore::new(pool.clone());
    let safe = Maturity {
        rating: MaturityRating::Safe,
        graphic: false,
    };
    let graphic_safe = Maturity {
        rating: MaturityRating::Safe,
        graphic: true,
    };
    let adult = Maturity {
        rating: MaturityRating::Adult,
        graphic: false,
    };
    let from_unrated = rating_of(&pool, &commission, adult).await;
    set_maturity(&pool, &commission, graphic_safe).await;
    let from_graphic = rating_of(&pool, &commission, adult).await;
    set_maturity(&pool, &commission, safe).await;

    let stale_unrated = write_rating(&pool, &commission, &from_unrated).await;
    let stale_graphic = write_rating(&pool, &commission, &from_graphic).await;

    assert!(!stale_unrated, "checked while unrated");
    assert!(!stale_graphic, "checked from the Graphic flag");
    let held = store
        .find(&commission.id)
        .await
        .expect("find")
        .expect("exists")
        .maturity;
    assert_eq!(held, Some(safe));
}

// The schema's teeth: a half-set posture (a rating with no graphic arm, or a
// graphic flag with no rating) violates the both-or-neither CHECK at the
// database — unrepresentable even for SQL that skips the domain.
#[tokio::test]
async fn a_half_set_posture_is_unrepresentable() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:check-owner").await;

    let rating_only = sqlx::query("UPDATE commission SET maturity = 'safe' WHERE id = $1")
        .bind(uuid::Uuid::from(commission.id))
        .execute(&pool)
        .await;
    assert!(
        rating_only
            .expect_err("a rating without its graphic arm must refuse")
            .to_string()
            .contains("commission_maturity_graphic_together"),
        "the CHECK names the violation",
    );

    let graphic_only = sqlx::query("UPDATE commission SET graphic = true WHERE id = $1")
        .bind(uuid::Uuid::from(commission.id))
        .execute(&pool)
        .await;
    assert!(
        graphic_only.is_err(),
        "a graphic flag without a rating must refuse",
    );
}

// The read gate: a token outside the enum's vocabulary in the column (row
// tampering / a missed migration) surfaces as an error on `find`, never a
// silent default — the same contract lifecycle and visibility carry.
#[tokio::test]
async fn a_tampered_maturity_token_surfaces_as_an_error() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:tamper-owner").await;
    let store = PgCommissionStore::new(pool.clone());

    sqlx::query("UPDATE commission SET maturity = 'explicit', graphic = false WHERE id = $1")
        .bind(uuid::Uuid::from(commission.id))
        .execute(&pool)
        .await
        .expect("tamper the row (the superseded vocabulary passes no CHECK — only the enum)");

    let err = store
        .find(&commission.id)
        .await
        .expect_err("an out-of-vocabulary token is an error, not a default");
    assert!(
        err.to_string().contains("maturity"),
        "the error names the tampered field, got: {err:?}",
    );
}
