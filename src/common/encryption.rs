use std::fmt::{Display};

use num::Num;
use rand_core::OsRng;
use rsa::{BigUint, PaddingScheme, PublicKey, PublicKeyParts, RSAPrivateKey, RSAPublicKey, errors::Error};
use serde::{Serialize, Deserialize};

pub struct Encryption{
    public_key: RSAPublicKey,
    secret_key: RSAPrivateKey
}
#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct NetworkedPublicKey {
    n: String,
    e: String
}

impl NetworkedPublicKey {
    pub fn recreate_public_key(&mut self) -> Result<RSAPublicKey, Error> {
        let n = BigUint::from_str_radix(&self.n, 36).unwrap();
        let e = BigUint::from_str_radix(&self.e, 36).unwrap();
        RSAPublicKey::new(n, e)
    }
}

impl Display for NetworkedPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.n[..10])
    }
}

impl Encryption {
    pub fn new() -> Encryption {
        let bits = 64; // FIXME: Set this to a sane amount once in 'production'
        let secret_key = RSAPrivateKey::new(&mut OsRng, bits).expect("Failed to generate a key");
        let public_key = RSAPublicKey::from(&secret_key);

        Encryption {
            secret_key,
            public_key
        }
    }

    pub fn get_public_key(&mut self) -> NetworkedPublicKey {
        NetworkedPublicKey {
            n: self.public_key.n().to_str_radix(36),
            e: self.public_key.e().to_str_radix(36)
        }
    }

    fn encrypt(public_key: RSAPublicKey, data: &[u8]) -> Vec<u8> {
        let padding = PaddingScheme::new_pkcs1v15_encrypt();
        let enc_data = public_key.encrypt(&mut OsRng, padding, &data[..]).expect("Failed to encrypt");
        enc_data
    }

    fn decrypt(&mut self, data: &[u8]) -> Vec<u8> {
        let padding = PaddingScheme::new_pkcs1v15_encrypt();
        let dec_data = self.secret_key.decrypt(padding, &data).expect("Failed to decrypt");
        dec_data
    }
}