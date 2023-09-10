use json_payload::json_payload;

#[json_payload]
struct Animal {
    name: String,
    age: u32,
}

#[allow(dead_code)]
fn main() {}
