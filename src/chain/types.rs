use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxReceipt {
    pub tx_id: String,
    pub status: bool,
    pub tx_index: u32,
    pub gas_used: u64,
    pub fee_paid: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub height: u64,
    pub slot: u64,
    pub epoch: u64,
    pub parent_hash: String,
    pub txs_root: String,
    pub receipts_root: String,
    pub timestamp: u64,
    pub proposer_pubkey_b64: String,
    pub signature_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub txs: Vec<crate::rpc::types::SignedTx>,
    pub receipts: Vec<TxReceipt>,
    pub hash: String,
}

pub fn header_signing_bytes(h: &BlockHeader) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(h.height.to_be_bytes());
    hasher.update(h.slot.to_be_bytes());
    hasher.update(h.epoch.to_be_bytes());
    hasher.update(h.parent_hash.as_bytes());
    hasher.update(h.txs_root.as_bytes());
    hasher.update(h.receipts_root.as_bytes());
    hasher.update(h.timestamp.to_be_bytes());
    hasher.update(h.proposer_pubkey_b64.as_bytes());
    hasher.finalize().to_vec()
}

pub fn header_hash(h: &BlockHeader) -> String {
    hex::encode(sha2::Sha256::digest(&header_signing_bytes(h)))
}

pub fn list_root_hex(strings: &[String]) -> String {
    if strings.is_empty() { return "0".repeat(64); }
    let mut layer: Vec<Vec<u8>> = strings.iter().map(|s| sha2::Sha256::digest(s.as_bytes()).to_vec()).collect();
    while layer.len() > 1 {
        let mut next = Vec::with_capacity((layer.len() + 1)/2);
        for pair in layer.chunks(2) {
            let a = &pair[0];
            let b = if pair.len()==2 { &pair[1] } else { &pair[0] };
            let mut h = Sha256::new();
            h.update(a); h.update(b);
            next.push(h.finalize().to_vec());
        }
        layer = next;
    }
    hex::encode(&layer[0])
}
