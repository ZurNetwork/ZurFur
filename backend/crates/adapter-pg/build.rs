// Ensure sqlx::migrate!() picks up new migration files on incremental builds.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
