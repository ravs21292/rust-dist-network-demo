use anyhow::Result;
use ed25519_dalek::{SigningKey, VerifyingKey, Signature};
use ed25519_dalek::Signer; // for .sign()
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

// Base64 engine (non-deprecated API)
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;

pub fn gen_keypair(label: &str) -> (SigningKey, VerifyingKey, [u8; 32]) {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let sk = SigningKey::from_bytes(&seed);
    let vk = VerifyingKey::from(&sk);

    println!("--- {label} KEYPAIR ---");
    println!("private seed (hex): {}", hex::encode(seed));
    println!("public key   (hex): {}", hex::encode(vk.as_bytes()));
    println!();

    (sk, vk, seed)
}

pub fn node_id(vk: &VerifyingKey) -> String {
    let mut h = Sha256::new();
    h.update(vk.as_bytes());
    hex::encode(h.finalize())
}

pub fn sign_nonce(sk: &SigningKey, nonce: &[u8]) -> String {
    let sig = sk.sign(nonce);
    B64.encode(sig.to_bytes())
}

pub fn verify_identity(nonce: &[u8], node_id_hex: &str, pubkey_b64: &str, sig_b64: &str) -> Result<()> {
    // decode peer public key (base64) -> VerifyingKey
    let pk_bytes = B64.decode(pubkey_b64.as_bytes())?;
    let pk_arr: [u8; 32] = pk_bytes.as_slice().try_into().map_err(|_| anyhow::anyhow!("bad pubkey len"))?;
    let vk = VerifyingKey::from_bytes(&pk_arr)?;

    // node_id must be sha256(pubkey)
    let expect = node_id(&vk);
    anyhow::ensure!(expect == node_id_hex, "node_id mismatch");

    // decode signature (base64) -> Signature
    let sig_bytes = B64.decode(sig_b64.as_bytes())?;
    let sig_arr: [u8; 64] = sig_bytes.as_slice().try_into().map_err(|_| anyhow::anyhow!("bad sig len"))?;
    let sig = Signature::from_bytes(&sig_arr);

    // verify
    vk.verify_strict(nonce, &sig).map_err(|e| anyhow::anyhow!("verify failed: {e}"))?;
    Ok(())
}