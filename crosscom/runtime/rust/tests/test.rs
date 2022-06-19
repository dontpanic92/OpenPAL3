use crate::crosscom_gen::ITest;
use crate::crosscom_gen::ITest2;
use crate::crosscom_gen::ITest2Impl;
use crate::crosscom_gen::ITest3;
use crate::crosscom_gen::ITest3Impl;
use crate::crosscom_gen::ITest4;
use crate::crosscom_gen::ITest4Impl;
use crate::crosscom_gen::ITestImpl;
use crosscom::ComRc;

#[macro_use]
mod crosscom_gen;

struct Test {
    pub test: Option<ComRc<ITest3>>,
}

ComObject_Test!(crate::Test);

impl Drop for Test {
    fn drop(&mut self) {
        println!("dropping");
    }
}

impl ITestImpl for Test {
    fn test(&self) -> i32 {
        42
    }
}

impl ITest2Impl for Test {
    fn mul(&self, a: i32, b: f32) -> f32 {
        a as f32 * b
    }
}

impl ITest3Impl for Test {
    fn echo(&self, a: i32) -> i32 {
        a
    }
}

impl ITest4Impl for Test {
    fn get(&self) -> ComRc<ITest3> {
        self.test.clone().unwrap()
    }
}

#[test]
fn test_basic() {
    let test = Test { test: None };
    let com_object = ComRc::<ITest>::from_object(test);
    let clone = com_object.clone();
    assert_eq!(42, com_object.test());
    assert_eq!(42, clone.test());

    let test2 = com_object.query_interface::<ITest2>().unwrap();
    assert_eq!(16., test2.mul(4, 4.));

    let test3 = test2.query_interface::<ITest3>().unwrap();
    assert_eq!(100, test3.echo(100));
}

#[test]
fn test_return_obj() {
    let inner = ComRc::<ITest3>::from_object(Test { test: None });
    assert_eq!(200, inner.echo(200));

    let test = Test { test: Some(inner) };

    let com_object = ComRc::<ITest4>::from_object(test);
    let itest3 = com_object.get();
    assert_eq!(300, itest3.echo(300));
}
