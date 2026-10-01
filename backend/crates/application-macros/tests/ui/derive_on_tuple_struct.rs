use application_macros::WithPorts;

#[derive(WithPorts)]
struct Probe<'a>(#[ports] &'a u8);

fn main() {}
