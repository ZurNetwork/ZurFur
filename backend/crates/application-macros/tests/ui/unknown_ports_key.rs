use application_macros::WithPorts;

#[derive(WithPorts)]
struct Probe<'a> {
    #[ports(root = true)]
    ports: &'a u8,
}

fn main() {}
