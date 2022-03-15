use crate::crosscom_gen::ITest;
use crate::crosscom_gen::ITestTrait;
use crosscom::{ComObject, ComRc};

#[macro_use]
mod crosscom_gen;

pub struct Test {}

implement_Test!(Test);

impl ITestTrait for Test {
    fn test(&self) {
        println!("In Rust!");
    }
}

#[test]
fn test_com() {
    let test = Test {};
    let com_object = ComRc::<ITest>::from_object(test);
    let object2 = com_object.clone();
    com_object.test();
}
