use anyhow::Result;
use crypto_box::{
    aead::{Aead, OsRng},
    Nonce, PublicKey, SecretKey,
};
use rand::RngCore;
use std::path::PathBuf;

type E2EEBox = crypto_box::SalsaBox;

pub struct ChatCrypto {
    secret: SecretKey,
    public: PublicKey,
}

impl ChatCrypto {
    pub fn load_or_generate(data_dir: &PathBuf) -> Result<Self> {
        let secret_path = data_dir.join("e2ee_secret.key");
        let public_path = data_dir.join("e2ee_public.key");

        if secret_path.exists() && public_path.exists() {
            let secret_vec = std::fs::read(&secret_path)?;
            let public_vec = std::fs::read(&public_path)?;
            let secret: [u8; 32] = secret_vec
                .try_into()
                .map_err(|_| anyhow::anyhow!("invalid secret key file"))?;
            let public: [u8; 32] = public_vec
                .try_into()
                .map_err(|_| anyhow::anyhow!("invalid public key file"))?;
            let secret = SecretKey::from(secret);
            let public = PublicKey::from(public);
            tracing::info!("loaded existing E2EE keys");
            Ok(Self { secret, public })
        } else {
            let secret = SecretKey::generate(&mut OsRng);
            let public = secret.public_key();
            std::fs::create_dir_all(data_dir)?;
            std::fs::write(&secret_path, secret.to_bytes())?;
            std::fs::write(&public_path, public.as_bytes())?;
            tracing::info!("generated new E2EE keys");
            Ok(Self { secret, public })
        }
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        *self.public.as_bytes()
    }

    pub fn encrypt(&self, recipient_pub: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
        let recipient = PublicKey::from(*recipient_pub);
        let box_ = E2EEBox::new(&recipient, &self.secret);
        let mut nonce_bytes = [0u8; 24];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from(nonce_bytes);
        let mut ciphertext = box_
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("encryption failed: {}", e))?;
        let mut result = nonce_bytes.to_vec();
        result.append(&mut ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, sender_pub: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < 24 {
            anyhow::bail!("ciphertext too short");
        }
        let sender = PublicKey::from(*sender_pub);
        let box_ = E2EEBox::new(&sender, &self.secret);
        let nonce_bytes: [u8; 24] = ciphertext[..24]
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid nonce length"))?;
        let nonce = Nonce::from(nonce_bytes);
        let plaintext = box_
            .decrypt(&nonce, &ciphertext[24..])
            .map_err(|e| anyhow::anyhow!("decryption failed: {}", e))?;
        Ok(plaintext)
    }
}
