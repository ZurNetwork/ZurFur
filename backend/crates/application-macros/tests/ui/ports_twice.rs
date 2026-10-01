use application_macros::WithPorts;

#[derive(WithPorts)]
struct Probe<'a> {
    #[ports]
    first: &'a u8,
    #[ports]
    second: &'a u8,
}

fn main() {}
