use std::collections::HashMap;


mod reader;
#[cfg(test)]
mod tests;


const TAG_END: u8 = 0;
const TAG_BYTE: u8 = 1;
const TAG_SHORT: u8 = 2;
const TAG_INT: u8 = 3;
const TAG_LONG: u8 = 4;
const TAG_FLOAT: u8 = 5;
const TAG_DOUBLE: u8 = 6;
const TAG_BYTE_ARRAY: u8 = 7;
const TAG_STRING: u8 = 8;
const TAG_LIST: u8 = 9;
const TAG_COMPOUND: u8 = 10;
const TAG_INT_ARRAY: u8 = 11;


#[derive(Debug)]
pub enum Value {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(List),
    Compound(Compound),
    IntArray(Vec<i32>),
}


/// The root value in NBT files has a name associated with it. It is almost
/// always the empty string.
pub struct RootValue {
    pub name: String,
    pub value: Value,
}


pub type Compound = HashMap<String, Value>;


#[derive(Debug)]
pub enum List {
    // Sometimes, TAG_Lists of size zero have an interal element type of
    // TAG_End. I.e., the list is a list of "TAG_End"s, but that makes no
    // sense. They're only consider valid at size zero, so there's no
    // associated vector.
    Empty,
    Byte(Vec<i8>),
    Short(Vec<i16>),
    Int(Vec<i32>),
    Long(Vec<i64>),
    Float(Vec<f32>),
    Double(Vec<f64>),
    ByteArray(Vec<Vec<u8>>),
    String(Vec<String>),
    List(Vec<List>),
    Compound(Vec<Compound>),
    IntArray(Vec<Vec<i32>>),
}
