mod jsonresponder_test;

#[test]
fn tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/jsonresponder_test.rs");
}
