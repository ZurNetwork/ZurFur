use application_macros::use_case;

struct Probe;

impl Probe {
    #[use_case]
    async fn run(&self, #[unit] first: u8, #[unit] second: u8) {}
}

fn main() {}
