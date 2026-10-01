use application_macros::WithPorts;

#[derive(WithPorts)]
struct Probe<'a> {
    #[ports(is_root = "yes")]
    ports: &'a u8,
}

fn main() {}
