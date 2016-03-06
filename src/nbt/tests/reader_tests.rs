use std::io::Cursor;

use ::nbt;
use ::nbt::reader;


const HELLO_WORLD: &'static [u8] = include_bytes!("hello_world.nbt");


#[test]
fn test_reader_hello_world() {
    let mut hello_world = Cursor::new(HELLO_WORLD);

    let root = match reader::parse_nbt_stream(&mut hello_world) {
        Ok(result) => result,
        Err(err) => panic!(err),
    };
    assert_eq!(root.name, "hello world");
    let root_value = match root.value {
        nbt::Value::Compound(c) => c,
        _ => panic!("Not a compound?"),
    };
    assert_eq!(1, root_value.len());
    let entry = match root_value.get("name") {
        None => panic!("Expected value not in Compound."),
        Some(v) => v,
    };
    match entry {
        &nbt::Value::String(ref s) => assert_eq!("Bananrama", s),
        _ => panic!("Entry wasn't a string."),
    };
}
