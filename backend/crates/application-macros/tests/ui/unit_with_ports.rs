use application_macros::use_case;

struct Probe;

impl Probe {
    #[use_case]
    async fn run(&self, #[unit] uow: u8, #[ports] ports: u8) {}
}

fn main() {}
