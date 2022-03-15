use std::str::FromStr;

use liquid::model::ScalarCow;
use liquid_core::{
    Display_filter, Filter, FilterReflection, ParseFilter, Runtime, Value, ValueView,
};


pub use rust::*;
mod rust;


#[derive(Clone, ParseFilter, FilterReflection)]
#[filter(
    name = "uuid_hex_array",
    description = "Converts a uuid to hex array presentation",
    parsed(UuidHexArrayFilter)
)]
pub struct UuidHexArray;

#[derive(Debug, Default, Display_filter)]
#[name = "uuid_hex_array"]
struct UuidHexArrayFilter;

impl Filter for UuidHexArrayFilter {
    fn evaluate(&self, input: &dyn ValueView, _runtime: &dyn Runtime) -> liquid_core::Result<Value> {
        if input.is_nil() {
            return Ok(Value::Nil);
        }

        let s = input.to_kstr();
        let uuid = uuid::Uuid::from_str(s.as_str()).unwrap();
        let sss = uuid.as_bytes().iter().map(|c| format!("0x{:02x?}", c)).collect::<Vec<String>>().join(", ");

        Ok(Value::Scalar(ScalarCow::from(sss)))
    }
}
