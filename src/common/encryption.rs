use std::fmt::Display;

use aes_gcm_siv::Aes256GcmSiv; // Or `Aes128GcmSiv`
use aes_gcm_siv::aead::{Aead, NewAead, generic_array::GenericArray};
use num::Num;
use rand_core::OsRng;
use rsa::{BigUint, PaddingScheme, PublicKey, PublicKeyParts, RSAPrivateKey, RSAPublicKey, errors::Error};
use serde::{Serialize, Deserialize};

pub struct AsymmetricEncryption{
    public_key: RSAPublicKey,
    secret_key: RSAPrivateKey,
}

impl AsymmetricEncryption {
    pub fn new() -> AsymmetricEncryption {
        let bits = 1024; // FIXME: Set this to a sane amount once in 'production'
        let secret_key = RSAPrivateKey::new(&mut OsRng, bits).expect("Failed to generate a key");
        let public_key = RSAPublicKey::from(&secret_key);

        AsymmetricEncryption {
            secret_key,
            public_key,
        }
    }

    pub fn get_public_key(&self) -> NetworkedPublicKey {
        NetworkedPublicKey {
            n: self.public_key.n().to_str_radix(36),
            e: self.public_key.e().to_str_radix(36)
        }
    }

    pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        let padding = PaddingScheme::new_oaep::<sha2::Sha256>();
        let dec_data = self.secret_key.decrypt(padding, &data).expect("Failed to decrypt");
        dec_data
    }
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct NetworkedPublicKey {
    n: String,
    e: String
}

impl NetworkedPublicKey {
    pub fn recreate_my_public_key(&self) -> Result<RSAPublicKey, Error> {
        let n = BigUint::from_str_radix(&self.n, 36).unwrap();
        let e = BigUint::from_str_radix(&self.e, 36).unwrap();
        RSAPublicKey::new(n, e)
    }

    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        let public_key = self.recreate_my_public_key().unwrap();
        let padding = PaddingScheme::new_oaep::<sha2::Sha256>();
        let enc_data = public_key.encrypt(&mut OsRng, padding, &data[..]).expect("Failed to encrypt");
        enc_data
    }
}

impl Display for NetworkedPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.n[..10])
    }
}

pub struct SymmetricEncryption {
    pub secret: Vec<u8>,
    sym_key: Aes256GcmSiv
}

impl SymmetricEncryption {
    pub fn new() -> SymmetricEncryption {
        let secret = (0..32).map(|_| { rand::random::<u8>() }).collect::<Vec<_>>();
        let secret_arr= GenericArray::from_slice(&secret[..]);
        let sym_key = Aes256GcmSiv::new(secret_arr);
        SymmetricEncryption {
            secret,
            sym_key
        }
    }

    pub fn new_from_secret(secret: &[u8]) -> SymmetricEncryption {
        let secret_arr= GenericArray::from_slice(&secret[..]);
        let sym_key = Aes256GcmSiv::new(secret_arr);
        SymmetricEncryption {
            secret: secret.to_vec(),
            sym_key
        }
    }

    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        let nonce = GenericArray::from_slice(b"123456789123".as_ref());
        self.sym_key.encrypt(nonce, data).unwrap()
    }

    pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        let nonce = GenericArray::from_slice(b"123456789123".as_ref());
        self.sym_key.decrypt(nonce, data).unwrap()
    }
}

