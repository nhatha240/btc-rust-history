use hex;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub fn sign(secret: &str, message: &str) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}
