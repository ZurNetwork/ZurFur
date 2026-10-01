use application_macros::use_case;

struct Probe;

impl Probe {
    #[use_case]
    async fn run(&self, (first, second): (u8, u8)) {}
}

fn main() {}
