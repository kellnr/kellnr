mod jsonpayload_test;

#[test]
fn tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/jsonpayload_test.rs");
}
