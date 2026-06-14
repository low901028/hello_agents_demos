use std::sync::{Arc, Mutex};
use serde::de::Unexpected::Option;

fn main() {
    let val = Some(Arc::new(Mutex::new(String::from("Hello, World!"))));
    println!("result = {:?}", val.unwrap_or(Arc::new(Mutex::new("demo".to_string()))));
}