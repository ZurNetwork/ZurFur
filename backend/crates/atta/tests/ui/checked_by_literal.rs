//! Outside Atta, a `Checked` can't be built by struct literal: only `Writer::act` makes one.

use atta::{Act, ActName, Checked, Level, Placed, RealPath, Shape, Type};

fn main() {
    let node = Placed {
        path: RealPath::default(),
        typ: Type::from("folder".to_owned()),
        level: Level::Public,
    };
    let act = Act {
        name: ActName::from("change".to_owned()),
        typ: Type::from("folder".to_owned()),
        shape: Shape::Change { node },
        body: (),
    };
    let _forged = Checked {
        act,
        settled: Vec::new(),
    };
}
