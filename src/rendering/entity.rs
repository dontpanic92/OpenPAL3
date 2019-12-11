
trait Entity {

}

struct RuntimeEntity {

}

impl Entity for RuntimeEntity {

}

pub fn create() -> Box<dyn Entity> {
    Box::new(RuntimeEntity {})
}
