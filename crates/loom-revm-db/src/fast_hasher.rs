use std::hash::{BuildHasher, Hash, Hasher};

use alloy::primitives::{Address, U256};

#[derive(Clone, Eq, PartialEq)]
pub struct HashedAddress(Address);

#[derive(Clone, Eq, PartialEq)]
pub struct HashedAddressCell(pub Address, pub U256);

impl From<Address> for HashedAddress {
    fn from(value: Address) -> Self {
        HashedAddress(value)
    }
}

impl Hash for HashedAddress {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut array = [0u8; 8];
        array.copy_from_slice(&self.0[0..8]);
        let mut value = u64::from_ne_bytes(array);
        array.copy_from_slice(&self.0[8..16]);
        value ^= u64::from_ne_bytes(array);
        let mut array = [0u8; 4];
        array.copy_from_slice(&self.0[16..20]);
        value += u32::from_ne_bytes(array) as u64;

        state.write_u64(value);
    }
}

impl Hash for HashedAddressCell {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut value = 0x12345678;
        let mut array = [0u8; 8];
        array.copy_from_slice(&self.0[0..8]);
        value += u64::from_ne_bytes(array);
        array.copy_from_slice(&self.0[8..16]);
        value ^= u64::from_ne_bytes(array);

        let u_slice = self.1.as_le_slice();

        array.copy_from_slice(&u_slice[0..8]);
        value += u64::from_ne_bytes(array);
        array.copy_from_slice(&u_slice[8..16]);
        value ^= u64::from_ne_bytes(array);
        array.copy_from_slice(&u_slice[16..24]);
        value += u64::from_ne_bytes(array);
        array.copy_from_slice(&u_slice[24..32]);
        value ^= u64::from_ne_bytes(array);

        let mut array = [0u8; 4];
        array.copy_from_slice(&self.0[16..20]);
        value += u32::from_ne_bytes(array) as u64;

        state.write_u64(value);
    }
}

#[derive(Default)]
pub struct SimpleHasher {
    state: u64,
}

impl SimpleHasher {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Hasher for SimpleHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.state
    }

    #[inline]
    fn write(&mut self, bytes_buf: &[u8]) {
        if self.state == 0 {
            self.state = 1;
        } else {
            let l = bytes_buf.len() / 8;
            let r = bytes_buf.len() % 8;

            let mut state: u64 = 0x12345678;
            let mut array = [0u8; 8];
            for i in 0..l {
                array.copy_from_slice(&bytes_buf[i * 8..i * 8 + 8]);
                let value = u64::from_ne_bytes(array);
                if i % 2 == 0 {
                    (state, _) = state.overflowing_add(value);
                } else {
                    state ^= value;
                }
            }
            if r != 0 {
                let mut array = [0u8; 8];
                for i in 0..r {
                    array[i] = bytes_buf[l * 8 + i]
                }
                let value = u64::from_ne_bytes(array);
                state += value;
            }

            // let mut array = [0u8; 8];
            // let mut state = 0x12345678;
            // array.copy_from_slice(&bytes_buf[0..8]);
            // let value = u64::from_ne_bytes(array);
            // state += value;
            // array.copy_from_slice(&bytes_buf[8..16]);
            // let value = u64::from_ne_bytes(array);
            // state ^= value;
            // let mut array = [0u8; 4];
            // array.copy_from_slice(&bytes_buf[16..20]);
            // let value = u32::from_ne_bytes(array);
            // state += value as u64;

            self.state = state
        }
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.state = i;
    }
}

#[derive(Default, Clone)]
pub struct SimpleBuildHasher {}

impl BuildHasher for SimpleBuildHasher {
    type Hasher = SimpleHasher;

    fn build_hasher(&self) -> Self::Hasher {
        SimpleHasher::new()
    }
}

#[cfg(test)]
mod test {
    use std::hash::Hash;

    use alloy::primitives::Address;

    use super::*;

    #[test]
    fn test_hasher() {
        let addr = Address::random();

        let mut hasher = SimpleHasher::new();
        addr.hash(&mut hasher);
    }

    #[test]
    fn test_hashed_address() {
        let addr = HashedAddress::from(Address::random());

        let mut hasher = SimpleBuildHasher {}.build_hasher();

        addr.hash(&mut hasher);
    }
}
