use application_macros::use_case;

struct Probe;

impl Probe {
    #[use_case]
    async fn run(&self, #[lazy_unit] first: u8, #[lazy_unit] second: u8) {}
}

fn main() {}
