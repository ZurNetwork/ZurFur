use std::str::FromStr;

use application::den::{node::NodeQuery, root::RootQuery};
use axum::{
    Json, Router,
    extract::{MatchedPath, State},
    http::{HeaderValue, Uri, header},
    routing::get,
};
use tower_http::set_header::SetResponseHeaderLayer;

use super::{errors::read_problem, params::DenParams, pieces::DenPieces};
use crate::{
    AppState,
    extract::CallingUser,
    generated::{GetDenNodeResponse, GetDenRootResponse},
    problem::Problem,
};

/// What every Den answer carries, the problems included.
const PRIVATE_NO_STORE: &str = "private, no-store";

/// The node route's prefix, stripped from the raw path before it is split.
const NODE_PREFIX: &str = "/den/";

/// The Den route group: the root, a node below it, and the empty path, which axum's
/// catch-all never matches and which answers the one not-found.
pub(crate) fn den_router() -> Router<AppState> {
    let private_no_store = SetResponseHeaderLayer::overriding(
        header::CACHE_CONTROL,
        HeaderValue::from_static(PRIVATE_NO_STORE),
    );
    Router::new()
        .route("/den", get(read_root))
        .route("/den/", get(read_node))
        .route("/den/{*path}", get(read_node))
        .layer(private_no_store)
}

/// The raw query string's parameters; no query is the default.
fn params(uri: &Uri) -> Result<DenParams, Problem> {
    let params = DenParams::from_str(uri.query().unwrap_or_default())?;
    Ok(params)
}

/// `GET /den` — the session viewer's own root and one page of its entries.
///
/// - `200` `GetDenRootResponse`
/// - `401 not_authenticated` — no session, before anything else
/// - `400 unknown_parameter`, `422 invalid_request` — the query, before the root is read
async fn read_root(
    State(state): State<AppState>,
    CallingUser(viewer): CallingUser,
    route: MatchedPath,
    uri: Uri,
) -> Result<Json<GetDenRootResponse>, Problem> {
    let params = params(&uri)?;
    let query = RootQuery {
        viewer,
        include_deleted: params.include_deleted,
        page: params.page,
    };
    let output = state
        .app()
        .den()
        .root(query)
        .await
        .map_err(|error| read_problem(error, route.as_str()))?;
    let body = GetDenRootResponse::from(output);
    Ok(Json(body))
}

/// `GET /den/{*path}` — the node at a Den path below the viewer's root. The raw path is split
/// on `/` first, then each piece is percent-decoded once.
///
/// - `200` `GetDenNodeResponse`
/// - `401 not_authenticated` — no session, before anything else
/// - `400 unknown_parameter`, `422 invalid_request` — the query, before the path is read
/// - `404 node_not_found` — every miss, malformed paths and the empty path included
async fn read_node(
    State(state): State<AppState>,
    CallingUser(viewer): CallingUser,
    route: MatchedPath,
    uri: Uri,
) -> Result<Json<GetDenNodeResponse>, Problem> {
    let params = params(&uri)?;
    let raw_path = uri
        .path()
        .strip_prefix(NODE_PREFIX)
        .ok_or_else(Problem::node_not_found)?;
    let pieces = DenPieces::from_str(raw_path)?;
    let query = NodeQuery {
        viewer,
        path: Vec::from(pieces),
        include_deleted: params.include_deleted,
        page: params.page,
    };
    let output = state
        .app()
        .den()
        .node(query)
        .await
        .map_err(|error| read_problem(error, route.as_str()))?;
    let body = GetDenNodeResponse::from(output);
    Ok(Json(body))
}
