use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub node_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Msg {
    // handshake (existing)
    Challenge { nonce_b64: String },
    Identity { name: String, node_id: String, pubkey_b64: String, sig_b64: String },
    Welcome { text: String },

    // membership gossip (existing)
    Peers { members: Vec<Member> },
    MemberUp { member: Member },
    MemberDown { node_id: String },

    // app messages (existing)
    App { from: String, text: String },
    Ack { text: String },

    // --- NEW: Transactions ---
    TxSubmit { tx: crate::rpc::types::SignedTx },
    TxAck    { id: String, ok: bool, err: Option<String> },
    TxAnnounce { tx: crate::rpc::types::SignedTx },
    // (Optional scalability later) TxInv/TxGet/TxBodies, TxSnapshot...

    // --- NEW: Blocks (execution payload) ---
    NewBlock { block: crate::chain::types::Block },
}
