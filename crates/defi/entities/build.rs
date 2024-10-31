use rand::{thread_rng, Rng};
use std::fs;

fn main() {
    let file_path = "./src/private.rs";
    if fs::metadata(file_path).is_err() {
        let mut rng = thread_rng();
        let password: Vec<u8> = (0..16).map(|_| rng.gen::<u8>()).collect();
        let password = format!("//{}\n\npub const KEY_ENCRYPTION_PWD: [u8; 16] = {:?};\n", hex::encode(password.clone()), password);

        let _ = fs::write(file_path, password);
    }
}
