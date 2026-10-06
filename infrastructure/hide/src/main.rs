use arboard::Clipboard;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use color_eyre::eyre::{Result, WrapErr, eyre};
use rsa::pkcs8::DecodePublicKey;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};

const PUBLIC_KEY: &str = include_str!("../public.key");

/// Encrypts the plaintext with PKCS#1 v1.5 padding and base64 encodes it, matching
/// `openssl rsautl -pubin -inkey public.key -encrypt | base64`.
fn encrypt(plaintext: &[u8]) -> Result<String> {
    let key = RsaPublicKey::from_public_key_pem(PUBLIC_KEY)
        .wrap_err("failed to parse embedded public key")?;

    let ciphertext = key
        .encrypt(&mut rand::thread_rng(), Pkcs1v15Encrypt, plaintext)
        .wrap_err("failed to encrypt plaintext")?;

    Ok(STANDARD.encode(ciphertext))
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let mut clipboard = Clipboard::new().wrap_err("failed to access the clipboard")?;
    let plaintext = clipboard
        .get_text()
        .wrap_err("failed to read text from the clipboard")?;

    // Copying from terminals/files often includes a trailing newline, which would
    // otherwise be encrypted and break validation after decryption.
    let plaintext = plaintext.trim_end_matches(['\n', '\r']);

    if plaintext.is_empty() {
        return Err(eyre!("the clipboard is empty"));
    }

    let encrypted = encrypt(plaintext.as_bytes())?;

    clipboard
        .set_text(encrypted)
        .wrap_err("failed to write to the clipboard")?;

    eprintln!("encrypted value copied to the clipboard");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_key_encrypts() {
        let out = encrypt(b"hunter2").unwrap();
        // 2048-bit key => 256 byte ciphertext
        assert_eq!(STANDARD.decode(out).unwrap().len(), 256);
    }

    #[test]
    fn rejects_oversized_plaintext() {
        assert!(encrypt(&[0u8; 300]).is_err());
    }
}
