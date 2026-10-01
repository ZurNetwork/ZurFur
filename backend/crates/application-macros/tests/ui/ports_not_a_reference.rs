use application_macros::WithPorts;

#[derive(WithPorts)]
struct Probe {
    #[ports]
    ports: u8,
}

fn main() {}
