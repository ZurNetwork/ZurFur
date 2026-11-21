//! An `OpenedSecret` can't be cloned: one opened secret is one copy, wiped on drop.

use kernel::elements::custody::OpenedSecret;

fn main() {
    let secret = OpenedSecret::from(b"secret".to_vec());
    let _copy = secret.clone();
}
