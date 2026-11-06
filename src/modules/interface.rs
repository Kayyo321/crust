/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use super::*;
use std::path::Path;

pub fn build(name: sym, exports: Vec<export_item>) -> module_interface {
    module_interface { name, hash: 0, exports }
}

pub fn encode(_i: &module_interface) -> Vec<u8> {
    b"CRMI".to_vec()
}

pub fn decode(_b: &[u8]) -> Result<module_interface, String> {
    Err("not implemented".into())
}

pub fn print_file(_p: &Path) -> Result<String, String> {
    Err("not implemented".into())
}
