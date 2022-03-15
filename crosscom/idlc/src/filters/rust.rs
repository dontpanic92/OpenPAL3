use liquid::model::ScalarCow;
use liquid_core::{
    Display_filter, Filter, FilterReflection, ParseFilter, Runtime, Value, ValueView,
};


#[derive(Clone, ParseFilter, FilterReflection)]
#[filter(
    name = "rust_raw_type",
    description = "Map a type to raw type",
    parsed(RustRawTypeFilter)
)]
pub struct RustRawType;

#[derive(Debug, Default, Display_filter)]
#[name = "rust_raw_type"]
struct RustRawTypeFilter;

impl Filter for RustRawTypeFilter {
    fn evaluate(&self, input: &dyn ValueView, runtime: &dyn Runtime) -> liquid_core::Result<Value> {
        if input.is_nil() {
            return Ok(Value::Nil);
        }

        let s = input.to_kstr();
        let k = s.as_str().to_string();
        
        println!("{:?}", runtime.get(&[ScalarCow::from("items.")]));
        Ok(Value::Scalar(ScalarCow::from(k)))
    }
}
