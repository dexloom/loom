use aes::cipher::{Block, BlockEncrypt, KeyInit};
use aes::Aes128;
use clap::{arg, Parser};
use eyre::Result;
use rand::{thread_rng, Rng};
use sha2::{Digest, Sha512};

use loom_types_entities::KeyStore;

const BLOCK_SIZE: usize = 16;

#[derive(Parser, Debug)]
enum Commands {
    GeneratePassword,
    Encrypt {
        #[arg(short, long)]
        key: String,
    },
}

fn encrypt_key(private_key: Vec<u8>, pwd: Vec<u8>) -> Vec<u8> {
    let mut hasher = Sha512::new();
    hasher.update(pwd.clone());
    let pwd_hash = hasher.finalize();

    let cipher = Aes128::new_from_slice(&pwd_hash[0..16]).unwrap();

    let mut ret = Vec::new();
    let mut block: Block<Aes128> = [0u8; BLOCK_SIZE].into();

    let mut a = 0;
    while a + BLOCK_SIZE <= private_key.len() {
        block.copy_from_slice(&private_key[a..a + BLOCK_SIZE]);
        cipher.encrypt_block(&mut block);
        ret.extend_from_slice(&block);
        a += BLOCK_SIZE;
    }

    let mut sha = Sha512::new();
    sha.update(&private_key);
    let crc = &sha.finalize()[0..4];

    ret.extend(crc);
    ret
}

fn main() -> Result<()> {
    let args = Commands::parse();
    match args {
        Commands::GeneratePassword => {
            let mut rng = thread_rng();
            let pwd: Vec<u8> = (0..BLOCK_SIZE).map(|_| rng.gen::<u8>()).collect();
            println!("{:?}", pwd);
        }
        Commands::Encrypt { key } => {
            let pwd = loom_types_entities::private::KEY_ENCRYPTION_PWD.to_vec();

            let private_key = hex::decode(key.strip_prefix("0x").unwrap_or(key.clone().as_str()))?;
            let encrypted_key = encrypt_key(private_key.clone(), pwd.clone());
            let keystore = KeyStore::new_from_bytes(pwd);
            let decrypted_key = keystore.encrypt_once(&encrypted_key.to_vec())?;
            if decrypted_key == private_key {
                println!("Encrypted private key : {}", hex::encode(encrypted_key))
            } else {
                println!("Error encrypting private key");
            }
        }
    }

    Ok(())
}
