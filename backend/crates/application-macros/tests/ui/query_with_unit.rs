use application_macros::use_case;

struct Query;
struct Probe;

impl Probe {
    #[use_case]
    async fn run(&self, #[lazy_unit] uow: u8, query: Query) {}
}

fn main() {}
