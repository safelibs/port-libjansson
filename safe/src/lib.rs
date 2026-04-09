#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types)]

pub mod abi;
pub mod array;
pub mod dump;
pub mod error;
pub mod load;
pub mod object;
pub mod raw {
    pub mod alloc;
    pub mod buf;
    pub mod list;
    pub mod table;
}
pub mod scalar;
pub mod strconv;
pub mod utf;
pub mod version;

pub use abi::{
    json_dump_callback_t, json_error_t, json_free_t, json_int_t, json_load_callback_t,
    json_malloc_t, json_t, json_type, JANSSON_MAJOR_VERSION, JANSSON_MICRO_VERSION,
    JANSSON_MINOR_VERSION, JSON_ARRAY, JSON_ERROR_SOURCE_LENGTH, JSON_ERROR_TEXT_LENGTH,
    JSON_FALSE, JSON_INTEGER, JSON_NULL, JSON_OBJECT, JSON_REAL, JSON_STRING, JSON_TRUE,
};
