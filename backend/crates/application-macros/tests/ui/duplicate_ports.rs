use application_macros::use_case;

struct Probe;

impl Probe {
    #[use_case]
    async fn run(&self, #[ports] first: u8, #[ports] second: u8) {}
}

fn main() {}
