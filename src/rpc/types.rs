use anyhow::Result;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use ed25519_dalek::Signer; // .sign()
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Minimal, deterministic, account-style TX for demo.
/// Canonical bytes are fixed-order, domain-separated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedTx {
    pub version: u16,
    pub chain_id: u32,
    pub from_pubkey_b64: String,
    pub nonce: u64,
    pub to: Option<String>,
    pub value: u128,
    pub max_fee: u64,
    pub data_b64: String,
    pub timestamp: u64,
    pub expiry: u64,
    pub sig_alg: String,      // "ed25519"
    pub sig_b64: String,
}

pub fn canonical_bytes(tx: &SignedTx) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);

    out.extend_from_slice(b"TX|v1|");
    out.extend_from_slice(&tx.chain_id.to_be_bytes());

    // from_pubkey_b64 -> bytes
    let from_pk = B64.decode(tx.from_pubkey_b64.as_bytes()).unwrap_or_default();
    out.extend_from_slice(&from_pk); // &[]

    out.extend_from_slice(&tx.nonce.to_be_bytes());

    if let Some(to) = &tx.to {
        out.extend_from_slice(to.as_bytes());
    }

    // value/max_fee
    out.extend_from_slice(&tx.value.to_be_bytes());   // u128 -> [u8;16]
    out.extend_from_slice(&tx.max_fee.to_be_bytes()); // u64  -> [u8;8]

    // data_b64 -> bytes
    let data = B64.decode(tx.data_b64.as_bytes()).unwrap_or_default();
    out.extend_from_slice(&data);

    // timestamps
    out.extend_from_slice(&tx.timestamp.to_be_bytes());
    out.extend_from_slice(&tx.expiry.to_be_bytes());

    out
}


pub fn tx_id(tx: &SignedTx) -> String {
    hex::encode(Sha256::digest(&canonical_bytes(tx)))
}

pub fn sign_tx(
    signing_key: &SigningKey,
    verifying_key: &VerifyingKey,
    chain_id: u32,
    nonce: u64,
    to: Option<String>,
    value: u128,
    max_fee: u64,
    data_b64: String,
    ttl_secs: u64,
) -> SignedTx {
    let ts = crate::consensus::clock::now_secs();
    let expiry = ts + ttl_secs;
    let from_pubkey_b64 = B64.encode(verifying_key.as_bytes());
    let mut tx = SignedTx {
        version: 1,
        chain_id,
        from_pubkey_b64,
        nonce,
        to,
        value,
        max_fee,
        data_b64,
        timestamp: ts,
        expiry,
        sig_alg: "ed25519".into(),
        sig_b64: String::new(),
    };
    let sig = signing_key.sign(&canonical_bytes(&tx));
    tx.sig_b64 = B64.encode(sig.to_bytes());
    tx
}

pub fn verify_tx(tx: &SignedTx) -> Result<()> {
    anyhow::ensure!(tx.sig_alg == "ed25519", "unsupported sig_alg");
    anyhow::ensure!(tx.expiry >= crate::consensus::clock::now_secs(), "expired");
    let pk_bytes = B64.decode(tx.from_pubkey_b64.as_bytes())?;
    let pk_arr: [u8;32] = pk_bytes.as_slice().try_into().map_err(|_| anyhow::anyhow!("bad pubkey len"))?;
    let vk = VerifyingKey::from_bytes(&pk_arr)?;
    let sig_bytes = B64.decode(tx.sig_b64.as_bytes())?;
    let sig_arr: [u8;64] = sig_bytes.as_slice().try_into().map_err(|_| anyhow::anyhow!("bad sig len"))?;
    let sig = Signature::from_bytes(&sig_arr);
    vk.verify_strict(&canonical_bytes(tx), &sig).map_err(|e| anyhow::anyhow!("verify failed: {e}"))?;
    Ok(())
}

/// Helper for demo CLI: random nonce
pub fn random_nonce() -> u64 { let mut b=[0u8;8]; OsRng.fill_bytes(&mut b); u64::from_be_bytes(b) }
