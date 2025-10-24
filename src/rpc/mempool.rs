use std::collections::{HashMap, HashSet};
use anyhow::Result;
use crate::rpc::types::{SignedTx, tx_id, verify_tx};

pub struct TxStore {
    pub by_id: HashMap<String, SignedTx>,
    pub seen:  HashSet<String>,
    pub max_seen_nonce: HashMap<String, u64>, // from_pubkey_b64 -> max nonce
    pub capacity: usize,
    pub min_fee: u64,
}

impl Default for TxStore {
    fn default() -> Self {
        Self {
            by_id: HashMap::new(),
            seen: HashSet::new(),
            max_seen_nonce: HashMap::new(),
            capacity: 50_000,
            min_fee: 0,
        }
    }
}

impl TxStore {
    pub fn insert_if_new(&mut self, tx: SignedTx) -> Result<(String, bool, String)> {
        verify_tx(&tx)?;
        anyhow::ensure!(tx.max_fee >= self.min_fee, "fee too low");
        let id = tx_id(&tx);
        if self.by_id.contains_key(&id) { return Ok((id, false, tx.from_pubkey_b64.clone())); }

        // light nonce policy
        let key = tx.from_pubkey_b64.clone();
        let maxn = *self.max_seen_nonce.get(&key).unwrap_or(&0);
        if tx.nonce < maxn { anyhow::bail!("nonce too low"); }
        self.max_seen_nonce.insert(key.clone(), tx.nonce.max(maxn));

        if self.by_id.len() >= self.capacity {
            if let Some(first) = self.by_id.keys().next().cloned() {
                self.by_id.remove(&first);
            }
        }
        self.seen.insert(id.clone());
        self.by_id.insert(id.clone(), tx);
        Ok((id, true, key))
    }

    pub fn drain_block(&mut self, max_count: usize) -> Vec<SignedTx> {
        let mut txs = Vec::new();
        for (id, tx) in self.by_id.iter().take(max_count).map(|(i,t)| (i.clone(), t.clone())).collect::<Vec<_>>() {
            self.by_id.remove(&id);
            txs.push(tx);
        }
        txs
    }

    pub fn ids_recent(&self, n: usize) -> Vec<String> {
        self.by_id.keys().take(n).cloned().collect()
    }
}
