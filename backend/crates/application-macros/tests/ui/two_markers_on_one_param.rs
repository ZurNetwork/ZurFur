use application_macros::use_case;

struct Probe;

impl Probe {
    #[use_case]
    async fn run(&self, #[ports] #[unit] both: u8) {}
}

fn main() {}
