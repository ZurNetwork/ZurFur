//! The Den read over HTTP: `GET /den` and `GET /den/{*path}` for the signed-in viewer's own
//! Den. The session is checked first, then the query, then the path, so no check depends on
//! what exists; every miss is the one `node_not_found`, and every answer is `private, no-store`.

mod errors;
mod params;
mod percent;
mod pieces;
mod router;
mod wire;

pub(crate) use router::den_router;
