use application_macros::use_case;

struct Probe;

impl Probe {
    #[use_case]
    async fn run(&self, #[unit] eager: u8, #[lazy_unit] lazy: u8) {}
}

fn main() {}
