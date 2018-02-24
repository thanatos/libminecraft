use std::convert::From;
use std::io;
use std::io::Read;
use std::mem;
use std::string;
use std::vec::Vec;


use super::{
    TAG_END,
    TAG_BYTE,
    TAG_SHORT,
    TAG_INT,
    TAG_LONG,
    TAG_FLOAT,
    TAG_DOUBLE,
    TAG_BYTE_ARRAY,
    TAG_STRING,
    TAG_LIST,
    TAG_COMPOUND,
    TAG_INT_ARRAY,
};

use super::{Value, RootValue, Compound, List};


#[derive(Debug)]
pub enum NbtReadError {
    UnexpectedEof,
    UnknownTagType(u8),
    InvalidTagType,
    IoError(io::Error),
    InvalidUtf8(string::FromUtf8Error),
}


fn tag_constant_to_name(tag_type: u8) -> String {
    String::from(match tag_type {
        TAG_END => "TAG_End",
        TAG_BYTE => "TAG_Byte",
        TAG_SHORT => "TAG_Short",
        TAG_INT => "TAG_Int",
        TAG_LONG => "TAG_Long",
        TAG_FLOAT => "TAG_Float",
        TAG_DOUBLE => "TAG_Double",
        TAG_BYTE_ARRAY => "TAG_Byte_Array",
        TAG_STRING => "TAG_String",
        TAG_LIST => "TAG_List",
        TAG_COMPOUND => "TAG_Compound",
        TAG_INT_ARRAY => "TAG_Int_Array",
        _ => return format!("(unknown tag type 0x{:02x})", tag_type),
    })
}


extern crate byteorder;
use self::byteorder::ReadBytesExt;


impl From<io::Error> for NbtReadError {
    fn from(err: io::Error) -> NbtReadError {
        NbtReadError::IoError(err)
    }
}


impl From<string::FromUtf8Error> for NbtReadError {
    fn from(err: string::FromUtf8Error) -> NbtReadError {
        NbtReadError::InvalidUtf8(err)
    }
}


macro_rules! read_number {
    ($reader:ident, $read_func:ident) => ({
        $reader.$read_func::<byteorder::BigEndian>()
            .map_err(NbtReadError::from)
    });
}

#[test]
fn test_read_signed() {
    let test_buf = vec![0u8, 1, 0, 0, 2, 0, 0xff, 0xff, 0xde];
    let mut cursor = io::Cursor::<Vec<u8>>::new(test_buf);

    assert!(0x01 == read_number!(cursor, read_i16).unwrap());
    assert!(0x200 == read_number!(cursor, read_i32).unwrap());
    assert!(-1 == read_number!(cursor, read_i16).unwrap());
    match read_number!(cursor, read_i16) {
        Ok(_) => panic!("Should have hit EOF, but didn't!"),
        Err(err) => match err {
            NbtReadError::UnexpectedEof => (),
            _ => panic!("Got unexpected error: {:?}", err),
        },
    };
}

#[test]
fn test_read_unsigned() {
    let test_buf = vec![3, 4, 0xfd, 0xfe];
    let mut cursor = io::Cursor::<Vec<u8>>::new(test_buf);
    assert!(0x304 == read_number!(cursor, read_u16).unwrap());
    assert!(0xfdfe == read_number!(cursor, read_u16).unwrap());
}


fn read_n_bytes_to_vector<R: ?Sized + Read>(reader: &mut R, length: usize)
        -> Result<Vec<u8>, NbtReadError> {
    let mut bytes = Vec::<u8>::with_capacity(length);
    unsafe { bytes.set_len(length); }
    let bytes_read = reader.read(&mut bytes[..])?;
    if bytes_read != length {
        Err(NbtReadError::UnexpectedEof)
    } else {
        Ok(bytes)
    }
}


struct UnknownTagType {
    tag_type: u8,
}


fn is_simple_value(tag_type: u8) -> Result<bool, UnknownTagType> {
    Ok(match tag_type {
        TAG_BYTE => true,
        TAG_SHORT => true,
        TAG_INT => true,
        TAG_LONG => true,
        TAG_FLOAT => true,
        TAG_DOUBLE => true,
        TAG_BYTE_ARRAY => true,
        TAG_STRING => true,
        TAG_LIST => false,
        TAG_COMPOUND => false,
        TAG_INT_ARRAY => false,
        _ => {
            return Err(UnknownTagType {
                tag_type: tag_type,
            });
        },
    })
}


fn read_nbt_string(reader: &mut Read) -> Result<String, NbtReadError> {
    // XXX: The NBT standard say "TAG_Short" for a length, which would imply
    // this length is signed. Which makes no sense.
    let length = read_number!(reader, read_u16)? as usize;
    let bytes = read_n_bytes_to_vector(reader, length)?;
    Ok(String::from_utf8(bytes)?)
}


fn read_nbt_byte_array(reader: &mut Read) -> Result<Vec<u8>, NbtReadError> {
    // XXX: The NBT standard say "TAG_Int" for a length, which would imply
    // this length is signed.  Which makes no sense.
    let length = read_number!(reader, read_u32)? as usize;
    read_n_bytes_to_vector(reader, length)
}


fn read_nbt_int_array(reader: &mut Read) -> Result<Vec<i32>, NbtReadError> {
    // XXX: The NBT standard say "TAG_Int" for a length, which would imply
    // this length is signed.  Which makes no sense.
    let length = read_number!(reader, read_u32)? as usize;
    let mut vec = Vec::<i32>::with_capacity(length);
    for _ in 0..length {
        vec.push(read_number!(reader, read_i32)?);
    }
    Ok(vec)
}


fn read_simple_value(tag_type: u8, reader: &mut Read)
        -> Result<Value, NbtReadError> {
    Ok(match tag_type {
        TAG_BYTE => Value::Byte(reader.read_i8()?),
        TAG_SHORT => Value::Short(read_number!(reader, read_i16)?),
        TAG_INT => Value::Int(read_number!(reader, read_i32)?),
        TAG_LONG => Value::Long(read_number!(reader, read_i64)?),
        TAG_FLOAT => Value::Float(read_number!(reader, read_f32)?),
        TAG_DOUBLE => Value::Double(read_number!(reader, read_f64)?),
        TAG_BYTE_ARRAY => Value::ByteArray(read_nbt_byte_array(reader)?),
        TAG_STRING => Value::String(read_nbt_string(reader)?),
        TAG_INT_ARRAY => Value::IntArray(read_nbt_int_array(reader)?),
        _ => panic!(
            "read_simple_value called for non-simple value {}",
            tag_constant_to_name(tag_type)
        ),
    })
}


enum ComplexReadResult {
    NotFinished,
    DescendInto(Box<ReadingComplex>),
    Done,
}


trait ReadingComplex {
    fn continue_read(&mut self, reader: &mut Read)
        -> Result<ComplexReadResult, NbtReadError>;
    fn descended_read_complete(&mut self, value: Value);
    fn final_value(self: Box<Self>) -> Value;
}


enum ReadStart {
    Simple(Value),
    Complex(Box<ReadingComplex>),
}


enum ListStart {
    Simple(List),
    ListOfList(ReadingListOfList),
    ListOfCompound(ReadingListOfCompound),
}


macro_rules! read_simple_list {
    (
        $list_enum_type: ident, $list_type:ty,
        $number_to_read:expr,
        $read_func:block
    ) => ({
        let mut the_list = Vec::<$list_type>::with_capacity($number_to_read);
        for _ in 0..$number_to_read {
            the_list.push(($read_func)?);
        }
        List::$list_enum_type(the_list)
    });
}


fn start_list_read(reader: &mut Read) -> Result<ListStart, NbtReadError> {
    let inner_tag_type = reader.read_u8()?;
    // XXX: The NBT standard say "TAG_Int" for a length, which would imply
    // this length is signed. Which makes no sense.
    let number = read_number!(reader, read_u32)? as usize;

    if inner_tag_type == TAG_END && number == 0 {
        return Ok(ListStart::Simple(List::Empty));
    }

    Ok(ListStart::Simple(match inner_tag_type {
        TAG_END => return Err(NbtReadError::InvalidTagType),
        TAG_BYTE => read_simple_list!(Byte, i8, number, { reader.read_i8() }),
        TAG_SHORT =>
            read_simple_list!(Short, i16, number, { read_number!(reader, read_i16) }),
        TAG_INT =>
            read_simple_list!(Int, i32, number, { read_number!(reader, read_i32) }),
        TAG_LONG =>
            read_simple_list!(Long, i64, number, { read_number!(reader, read_i64) }),
        TAG_FLOAT =>
            read_simple_list!(Float, f32, number, { read_number!(reader, read_f32) }),
        TAG_DOUBLE =>
            read_simple_list!(Double, f64, number, { read_number!(reader, read_f64) }),
        TAG_BYTE_ARRAY => read_simple_list!(
            ByteArray, Vec<u8>, number, { read_nbt_byte_array(reader) }
        ),
        TAG_STRING => read_simple_list!(
            String, String, number, { read_nbt_string(reader) }
        ),
        TAG_LIST => return Ok(ListStart::ListOfList(ReadingListOfList {
            items_remaining: number,
            value: Vec::<List>::new(),
        })),
        TAG_COMPOUND => return Ok(ListStart::ListOfCompound(ReadingListOfCompound {
            items_remaining: number,
            value: Vec::<Compound>::new(),
        })),
        TAG_INT_ARRAY => read_simple_list!(
            IntArray, Vec<i32>, number, { read_nbt_int_array(reader) }
        ),
        _ => return Err(NbtReadError::UnknownTagType(inner_tag_type)),
    }))
}


/**
 * Start reading a tag's value, where the value might be simple (TAG_INT) or complex
 * (TAG_COMPOUND).
 */
fn start_potentially_complex_read(tag_type: u8, reader: &mut Read)
        -> Result<ReadStart, NbtReadError> {
    let is_simple_tag = match is_simple_value(tag_type) {
        Ok(is_it) => is_it,
        Err(_) => return Err(NbtReadError::UnknownTagType(tag_type)),
    };
    if is_simple_tag {
        return Ok(
            ReadStart::Simple(read_simple_value(tag_type, reader)?)
        );
    }
    match tag_type {
        TAG_LIST => return Ok(
            match start_list_read(reader)? {
                ListStart::Simple(list) =>
                    ReadStart::Simple(Value::List(list)),
                ListStart::ListOfList(reading) =>
                    ReadStart::Complex(Box::new(reading)),
                ListStart::ListOfCompound(reading) =>
                    ReadStart::Complex(Box::new(reading)),
            }
        ),
        TAG_COMPOUND => {
            Ok(ReadStart::Complex(Box::new(ReadingCompound {
                value: Compound::new(),
                name_of_current_value: None,
            })))
        },
        _ => panic!(
            "Got a non-simple tag type {}, but it wasn't a compound or list?",
            tag_type,
        ),
    }
}


struct ReadingCompound {
    value: Compound,
    name_of_current_value: Option<String>,
}


impl ReadingComplex for ReadingCompound {
    fn continue_read(&mut self, reader: &mut Read)
            -> Result<ComplexReadResult, NbtReadError> {
        loop {
            let tag_type = reader.read_u8()?;
            if tag_type == TAG_END {
                return Ok(ComplexReadResult::Done);
            }

            let tag_name = read_nbt_string(reader)?;

            let maybe_complex_read = start_potentially_complex_read(
                tag_type, reader,
            )?;
            match maybe_complex_read {
                ReadStart::Simple(value) => {
                    self.value.insert(tag_name, value);
                },
                ReadStart::Complex(read_complex) => {
                    self.name_of_current_value = Some(tag_name);
                    return Ok(ComplexReadResult::DescendInto(read_complex));
                }
            }
        }
    }

    fn descended_read_complete(&mut self, value: Value) {
        let mut name = None;
        mem::swap(&mut name, &mut self.name_of_current_value);
        self.value.insert(name.unwrap(), value);
    }

    fn final_value(self: Box<Self>) -> Value {
        Value::Compound(self.value)
    }
}


struct ReadingListOfList {
    items_remaining: usize,
    value: Vec<List>,
}


impl ReadingComplex for ReadingListOfList {
    fn continue_read(&mut self, reader: &mut Read)
            -> Result<ComplexReadResult, NbtReadError> {
        if self.items_remaining == 0 {
            return Ok(ComplexReadResult::Done);
        }

        let maybe_complex_read = start_potentially_complex_read(
            TAG_LIST, reader
        )?;
        self.items_remaining -= 1;
        match maybe_complex_read {
            ReadStart::Simple(inner_value) => {
                if let Value::List(inner_list) = inner_value {
                    self.value.push(inner_list);
                } else {
                    panic!(
                        "During a complex list read, the inner value we got \
                         back wasn't a List. (But it should be.)"
                    );
                }
            },
            ReadStart::Complex(reading_complex) => {
                return Ok(ComplexReadResult::DescendInto(reading_complex));
            },
        }
        Ok(ComplexReadResult::NotFinished)
    }

    fn descended_read_complete(&mut self, inner_value: Value) {
        if let Value::List(inner_list) = inner_value {
            self.value.push(inner_list);
        } else {
            panic!(
                "ReadingListOfList::descended_read_complete got back a \
                 non-list."
            );
        }
    }

    fn final_value(self: Box<Self>) -> Value {
        Value::List(List::List(self.value))
    }
}


struct ReadingListOfCompound {
    items_remaining: usize,
    value: Vec<Compound>,
}


impl ReadingComplex for ReadingListOfCompound {
    fn continue_read(&mut self, reader: &mut Read)
            -> Result<ComplexReadResult, NbtReadError> {
        if self.items_remaining == 0 {
            return Ok(ComplexReadResult::Done);
        }

        let maybe_complex_read = start_potentially_complex_read(
            TAG_COMPOUND, reader
        )?;
        self.items_remaining -= 1;
        match maybe_complex_read {
            ReadStart::Simple(inner_value) => {
                panic!(
                    "During a continue_read for a list of compounds, got the \
                     unexpected simple value {:?}.",
                    inner_value,
                );
            },
            ReadStart::Complex(reading_complex) => {
                return Ok(ComplexReadResult::DescendInto(reading_complex));
            },
        }
    }

    fn descended_read_complete(&mut self, inner_value: Value) {
        if let Value::Compound(inner_compound) = inner_value {
            self.value.push(inner_compound);
        } else {
            panic!(
                "ReadingListOfCompound::descended_read_complete got back a \
                 non-compound."
            );
        }
    }

    fn final_value(self: Box<Self>) -> Value {
        Value::List(List::Compound(self.value))
    }
}


pub fn parse_nbt_stream(reader: &mut Read) -> Result<RootValue, NbtReadError> {
    let root_tag_type = reader.read_u8()?;
    let root_tag_name = read_nbt_string(reader)?;

    let read_start = start_potentially_complex_read(root_tag_type, reader)?;
    let reading = match read_start {
        ReadStart::Simple(value) => return Ok(RootValue {
            name: root_tag_name,
            value: value,
        }),
        ReadStart::Complex(reading_) => reading_,
    };
    let mut in_progress_reads = Vec::<Box<ReadingComplex>>::new();
    in_progress_reads.push(reading);

    loop {
        let result = {
            let working_read = in_progress_reads.last_mut().unwrap();
            working_read.continue_read(reader)?
        };
        match result {
            ComplexReadResult::NotFinished => (),
            ComplexReadResult::DescendInto(next_read) => {
                in_progress_reads.push(next_read);
            },
            ComplexReadResult::Done => {
                let complete_read = in_progress_reads.pop().unwrap();
                let value = complete_read.final_value();
                match in_progress_reads.last_mut() {
                    Some(working_read) => {
                        working_read.descended_read_complete(value);
                    },
                    None => {
                        return Ok(RootValue {
                            name: root_tag_name,
                            value: value,
                        });
                    },
                };
            },
        }
    }
}
