use application_macros::use_case;

struct Probe;

impl Probe {
    #[use_case]
    fn run(&self) {}
}

fn main() {}
