use derive_jsonresponder::JsonResponder;
use rocket::serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, JsonResponder)]
struct Animal {
    name: String,
    age: u32,
}

#[allow(dead_code)]
fn main() {}
