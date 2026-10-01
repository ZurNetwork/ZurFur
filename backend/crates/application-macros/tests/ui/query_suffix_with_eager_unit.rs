use application_macros::use_case;

struct MeQuery;
struct Probe;

impl Probe {
    #[use_case]
    async fn run(&self, #[unit] uow: u8, query: MeQuery) {}
}

fn main() {}
