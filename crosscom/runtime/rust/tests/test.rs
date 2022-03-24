use crate::crosscom_gen::ITest;
use crate::crosscom_gen::ITest2Trait;
use crate::crosscom_gen::ITestTrait;
use crosscom::ComRc;

#[macro_use]
mod crosscom_gen;

pub struct Test {}

implement_Test!(crate::Test);

impl ITestTrait for Test {
    fn test(&self) {
        println!("In Rust!");
    }
}

impl ITest2Trait for Test {
    fn test2(&self) -> () {
        println!("In Rust2!");
    }
}

#[test]
fn test_com() {
    let test = Test {};
    let com_object = ComRc::<ITest>::from_object(test);
    let _ = com_object.clone();
    com_object.test();
}
