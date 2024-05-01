use alloy_primitives::{Bytes, keccak256};

fn sel(s: &str) -> [u8; 4] {
    keccak256(s)[..4].try_into().unwrap()
}

pub fn match_abi(code: &Bytes, selectors: Vec<[u8; 4]>) -> bool {
    //println!("Code len {}", code.len());
    for selector in selectors.iter() {
        if !code.as_ref().windows(4).any(|sig| sig == selector) {
            //println!("{:?} not found", selector);
            return false;
        } else {
            //println!("{} found", fn_name);
        }
    }
    true
}