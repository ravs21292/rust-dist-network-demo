// use anyhow::{Context, Result};
// use std::{
//     collections::{HashMap, VecDeque},
//     net::SocketAddr,
//     sync::Arc,
// };
// use tokio::{
//     io::BufReader,
//     net::{TcpListener, TcpStream},
//     sync::{mpsc, RwLock},
// };

// use ed25519_dalek::{SigningKey, VerifyingKey, Signature};
// use ed25519_dalek::Signer; // brings .sign(&[u8]) into scope
// use rand::{rngs::OsRng, RngCore};
// use base64::engine::general_purpose::STANDARD as B64;
// use base64::Engine as _;

// use axum::{
//     extract::{Path, State as AxumState},
//     routing::{get, post},
//     Json, Router,
// };

// use crate::{
//     consensus::clock::{SlotClock, now_secs, sleep_until, EPOCH_SLOTS},
//     crypto::{gen_keypair, node_id, sign_nonce, verify_identity},
//     io::{recv, send},
//     protocol::{Msg, Member},
//     rpc::mempool::TxStore,
//     rpc::types::{SignedTx, tx_id, sign_tx, random_nonce},
//     chain::types::{Block, BlockHeader, TxReceipt, header_hash, header_signing_bytes, list_root_hex},
// };

// // ============================ Node State ============================

// struct PeerEntry {
//     name: String,
//     tx: mpsc::UnboundedSender<Msg>,
// }

// const BLOCK_BUFFER: usize = 256;

// #[derive(Default)]
// struct State {
//     peers: HashMap<String, PeerEntry>, // node_id -> entry
//     txs:   TxStore,
//     // chain tip/finality
//     tip_hash: String,
//     tip_height: u64,
//     finalized_height: u64,             // epoch-based finality
//     // recent blocks for HTTP
//     blocks: VecDeque<Block>,
// }

// // Context used by HTTP handlers
// #[derive(Clone)]
// struct HttpCtx {
//     state: Arc<RwLock<State>>,
//     sk: SigningKey,
//     vk: VerifyingKey,
// }

// // ============================ Public API (HTTP) ============================

// #[derive(serde::Deserialize)]
// struct SubmitTxBody {
//     // Option 1: a fully signed tx (wallet flow)
//     tx: Option<SignedTx>,
//     // Option 2: convenience path for quick testing – server signs a demo tx
//     message: Option<String>,
// }

// #[derive(serde::Serialize)]
// struct SubmitTxResp {
//     id: String,
//     ok: bool,
//     err: Option<String>,
//     inserted: bool,        // new to mempool?
//     mempool_size: usize,   // mempool size after insert
//     time_to_next_slot_ms: u64, // <-- NEW: ms until next slot boundary
// }

// #[derive(serde::Serialize)]
// struct MempoolEntry {
//     id: String,
//     from: String,
//     nonce: u64,
//     value: u128,
//     max_fee: u64,
//     timestamp: u64,
// }

// #[derive(serde::Serialize)]
// struct BlockListEntry {
//     height: u64,
//     hash: String,
//     slot: u64,
//     epoch: u64,
//     txs: usize,
//     timestamp: u64,
// }

// #[derive(serde::Serialize)]
// struct DebugState {
//     tip_height: u64,
//     finalized_height: u64,
//     mempool_size: usize,
//     // NEW:
//     slot: u64,
//     epoch: u64,
//     next_slot_in_ms: u64,
// }

// async fn post_tx(
//     AxumState(ctx): AxumState<HttpCtx>,
//     Json(body): Json<SubmitTxBody>,
// ) -> Json<SubmitTxResp> {
//     // Build tx
//     let maybe_tx = if let Some(tx) = body.tx {
//         Some(tx)
//     } else if let Some(msg) = body.message {
//         // demo: sign a tx with server's key so you can curl quickly
//         Some(sign_tx(
//             &ctx.sk,
//             &ctx.vk,
//             1,
//             random_nonce(),
//             None,
//             0,
//             1,
//             base64::encode(msg.as_bytes()),
//             600,
//         ))
//     } else {
//         None
//     };

//     // timing hint for client
//     let clock = SlotClock::new_aligned_now();
//     let next = clock.next_slot_start();
//     let now  = now_secs();
//     let time_to_next_slot_ms = if next > now { (next - now) * 1000 } else { 0 };

//     if maybe_tx.is_none() {
//         return Json(SubmitTxResp {
//             id: "".into(),
//             ok: false,
//             err: Some("provide `tx` or `message`".into()),
//             inserted: false,
//             mempool_size: 0,
//             time_to_next_slot_ms,
//         });
//     }
//     let tx = maybe_tx.unwrap();
//     let id = tx_id(&tx);

//     let mut inserted = false;
//     let mut err_str = None;
//     let mut to_gossip: Option<SignedTx> = None;
//     let mut mempool_size = 0usize;

//     {
//         let mut st = ctx.state.write().await;
//         match st.txs.insert_if_new(tx.clone()) {
//             Ok((_id, ins, _from)) => {
//                 inserted = ins;
//                 mempool_size = st.txs.by_id.len();
//                 println!(
//                     "[HTTP] TxSubmit {} accepted={} mempool={}",
//                     &id[..8], inserted, mempool_size
//                 );
//                 if ins {
//                     to_gossip = Some(tx.clone());
//                 }
//             }
//             Err(e) => {
//                 err_str = Some(e.to_string());
//                 mempool_size = st.txs.by_id.len();
//                 println!("[HTTP] TxSubmit {} rejected: {}", &id[..8], e);
//             }
//         }
//         if let Some(tx_announce) = to_gossip {
//             for (_peer, entry) in st.peers.iter() {
//                 let _ = entry.tx.send(Msg::TxAnnounce {
//                     tx: tx_announce.clone(),
//                 });
//             }
//         }
//     }

//     Json(SubmitTxResp {
//         id,
//         ok: err_str.is_none(),
//         err: err_str,
//         inserted,
//         mempool_size,
//         time_to_next_slot_ms,
//     })
// }

// async fn get_mempool(AxumState(ctx): AxumState<HttpCtx>) -> Json<Vec<MempoolEntry>> {
//     let st = ctx.state.read().await;
//     let mut out = Vec::new();
//     for (id, tx) in st.txs.by_id.iter() {
//         out.push(MempoolEntry {
//             id: id.clone(),
//             from: tx.from_pubkey_b64.clone(),
//             nonce: tx.nonce,
//             value: tx.value,
//             max_fee: tx.max_fee,
//             timestamp: tx.timestamp,
//         });
//     }
//     Json(out)
// }

// async fn get_blocks(AxumState(ctx): AxumState<HttpCtx>) -> Json<Vec<BlockListEntry>> {
//     let st = ctx.state.read().await;
//     let mut out = Vec::new();
//     for b in st.blocks.iter().rev() {
//         out.push(BlockListEntry {
//             height: b.header.height,
//             hash: b.hash.clone(),
//             slot: b.header.slot,
//             epoch: b.header.epoch,
//             txs: b.txs.len(),
//             timestamp: b.header.timestamp,
//         });
//     }
//     Json(out)
// }

// async fn get_block_by_height(
//     AxumState(ctx): AxumState<HttpCtx>,
//     Path(height): Path<u64>,
// ) -> Result<Json<Block>, axum::http::StatusCode> {
//     let st = ctx.state.read().await;
//     if let Some(b) = st.blocks.iter().find(|b| b.header.height == height) {
//         return Ok(Json(b.clone()));
//     }
//     Err(axum::http::StatusCode::NOT_FOUND)
// }

// async fn get_debug(AxumState(ctx): AxumState<HttpCtx>) -> Json<DebugState> {
//     let clock = SlotClock::new_aligned_now();
//     let now = now_secs();
//     let next = clock.next_slot_start();
//     let slot = clock.now_slot();
//     let epoch = clock.epoch_of_slot(slot);

//     let st = ctx.state.read().await;
//     Json(DebugState {
//         tip_height: st.tip_height,
//         finalized_height: st.finalized_height,
//         mempool_size: st.txs.by_id.len(),
//         slot,
//         epoch,
//         next_slot_in_ms: if next > now { (next - now) * 1000 } else { 0 },
//     })
// }

// fn spawn_http(ctx: HttpCtx, addr: &str) {
//     let app = Router::new()
//         .route("/api/tx", post(post_tx))
//         .route("/api/mempool", get(get_mempool))
//         .route("/api/blocks", get(get_blocks))
//         .route("/api/blocks/height/:height", get(get_block_by_height))
//         .route("/api/debug", get(get_debug))
//         .with_state(ctx);

//     let addr: SocketAddr = addr.parse().expect("bad --http-addr");
//     tokio::spawn(async move {
//         println!("[HTTP] listening on http://{}", addr);
//         if let Err(e) = axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app).await {
//             eprintln!("[HTTP] server error: {e}");
//         }
//     });
// }

// // ============================ TCP Node (existing) ============================

// pub async fn run_server(port: u16, name: String) -> Result<()> {
//     // Bootnode keys
//     let (my_sk, my_vk, _seed) = gen_keypair("NODE1 (server)");
//     let my_id = node_id(&my_vk);

//     // Derive human-readable name if "auto"
//     let mut my_name = name;
//     if my_name == "auto" {
//         my_name = format!("node-{}", &my_id[..8]);
//     }

//     let bind: SocketAddr = format!("0.0.0.0:{port}").parse().unwrap();
//     println!(
//         "[SERVER] listening on {bind}  id={}  name={}",
//         &my_id[..8], my_name
//     );

//     let listener = TcpListener::bind(bind).await?;
//     let state = Arc::new(RwLock::new(State {
//         peers: HashMap::new(),
//         txs: TxStore::default(),
//         tip_hash: "GENESIS".into(),
//         tip_height: 0,
//         finalized_height: 0, // epoch-based
//         blocks: VecDeque::with_capacity(BLOCK_BUFFER),
//     }));

//     // Start HTTP API on 127.0.0.1:9000 (change to "0.0.0.0:9000" if you need remote access)
//     spawn_http(
//         HttpCtx {
//             state: Arc::clone(&state),
//             sk: my_sk.clone(),
//             vk: my_vk.clone(),
//         },
//         "127.0.0.1:9000",
//     );

//     // Spawn block producer aligned to slots
//     {
//         let state_cloned = Arc::clone(&state);
//         let my_sk = my_sk.clone();
//         let my_vk = my_vk.clone();
//         tokio::spawn(async move {
//             run_block_producer(state_cloned, my_sk, my_vk).await;
//         });
//     }

//     loop {
//         let (stream, peer) = listener.accept().await?;
//         println!("[SERVER] incoming from {peer}");

//         let my_sk = my_sk.clone();
//         let my_vk = my_vk.clone();
//         let my_id = my_id.clone();
//         let my_name_cloned = my_name.clone();
//         let state_cloned = Arc::clone(&state);

//         tokio::spawn(async move {
//             if let Err(e) =
//                 handle_server_conn(stream, my_name_cloned, my_sk, my_vk, my_id, state_cloned).await
//             {
//                 eprintln!("[SERVER] error: {e:?}");
//             }
//         });
//     }
// }

// async fn run_block_producer(state: Arc<RwLock<State>>, sk: SigningKey, vk: VerifyingKey) {
//     const MAX_TXS_PER_BLOCK: usize = 100;
//     let clock = SlotClock::new_aligned_now();
//     loop {
//         // wait until next slot boundary
//         let next = clock.next_slot_start();
//         sleep_until(next).await;

//         // propose block at this slot
//         let slot = clock.now_slot();
//         let epoch = clock.epoch_of_slot(slot);

//         // drain txs from mempool
//         let (parent_hash, height, txs) = {
//             let mut st = state.write().await;
//             let txs = st.txs.drain_block(MAX_TXS_PER_BLOCK);
//             let h = st.tip_height + 1;
//             let ph = st.tip_hash.clone();
//             (ph, h, txs)
//         };

//         // build receipts (demo: all success), roots
//         let receipts: Vec<TxReceipt> = txs
//             .iter()
//             .enumerate()
//             .map(|(i, tx)| TxReceipt {
//                 tx_id: tx_id(tx),
//                 status: true,
//                 tx_index: i as u32,
//                 gas_used: 21_000,
//                 fee_paid: tx.max_fee,
//             })
//             .collect();

//         let tx_ids: Vec<String> = receipts.iter().map(|r| r.tx_id.clone()).collect();
//         let txs_root = list_root_hex(&tx_ids);
//         let receipts_root = list_root_hex(&tx_ids);

//         let header = BlockHeader {
//             height,
//             slot,
//             epoch,
//             parent_hash,
//             txs_root,
//             receipts_root,
//             timestamp: now_secs(),
//             proposer_pubkey_b64: B64.encode(vk.as_bytes()),
//             signature_b64: String::new(),
//         };

//         // sign header
//         let signing_bytes = header_signing_bytes(&header);
//         let sig = sk.sign(&signing_bytes);
//         let mut header = header;
//         header.signature_b64 = B64.encode(sig.to_bytes());

//         let hash = header_hash(&header);
//         let block = Block {
//             header: header.clone(),
//             txs: txs.clone(),
//             receipts: receipts.clone(),
//             hash: hash.clone(),
//         };

//         // update tip, epoch-based finality, store block
//         let finalized_h = {
//             let mut st = state.write().await;
//             st.tip_hash = hash.clone();
//             st.tip_height = height;
//             st.blocks.push_back(block.clone());
//             if st.blocks.len() > BLOCK_BUFFER {
//                 st.blocks.pop_front();
//             }

//             // finalize at epoch boundaries:
//             // when entering a new epoch (slot % EPOCH_SLOTS == 0), finalize last block of (epoch-2)
//             let slot_in_epoch = slot % EPOCH_SLOTS;
//             if slot_in_epoch == 0 {
//                 if epoch >= 2 {
//                     st.finalized_height = (epoch - 2 + 1) * EPOCH_SLOTS;
//                 } else {
//                     st.finalized_height = 0;
//                 }
//             }
//             st.finalized_height
//         };

//         println!(
//             "[PROPOSER] block h={} slot={} epoch={} txs={} finalized_h={}",
//             height,
//             slot,
//             epoch,
//             block.txs.len(),
//             finalized_h
//         );

//         // broadcast to all peers
//         let peers = {
//             let st = state.read().await;
//             st.peers
//                 .values()
//                 .map(|p| p.tx.clone())
//                 .collect::<Vec<_>>()
//         };
//         for tx in peers {
//             let _ = tx.send(Msg::NewBlock {
//                 block: block.clone(),
//             });
//         }
//     }
// }

// async fn handle_server_conn(
//     stream: TcpStream,
//     my_name: String,
//     my_sk: SigningKey,
//     my_vk: VerifyingKey,
//     my_id: String,
//     state: Arc<RwLock<State>>,
// ) -> Result<()> {
//     let (read_half, mut write_half) = stream.into_split();
//     let mut reader = BufReader::new(read_half);

//     // --- HANDSHAKE ---
//     // 1) send challenge
//     let mut nonce = [0u8; 32];
//     OsRng.fill_bytes(&mut nonce);
//     println!("[SERVER] sending Challenge nonce={}", hex::encode(nonce));
//     send(
//         &mut write_half,
//         &Msg::Challenge {
//             nonce_b64: B64.encode(nonce),
//         },
//     )
//     .await?;

//     // 2) receive client's Identity and verify
//     let (peer_name, peer_id) = match recv(&mut reader).await? {
//         Msg::Identity {
//             name,
//             node_id,
//             pubkey_b64,
//             sig_b64,
//         } => {
//             println!(
//                 "[SERVER] verifying client Identity: name={name}, node_id={}...",
//                 &node_id[..8]
//             );
//             verify_identity(&nonce, &node_id, &pubkey_b64, &sig_b64)
//                 .context("client identity verification failed")?;
//             println!("[SERVER] ✅ client authed: name={name} id={}", &node_id[..8]);
//             (name, node_id)
//         }
//         other => anyhow::bail!("expected Identity, got {other:?}"),
//     };

//     // 3) send our Identity (mutual auth) + Welcome
//     let sig_b64 = sign_nonce(&my_sk, &nonce);
//     send(
//         &mut write_half,
//         &Msg::Identity {
//             name: my_name.clone(),
//             node_id: my_id.clone(),
//             pubkey_b64: B64.encode(my_vk.as_bytes()),
//             sig_b64,
//         },
//     )
//     .await?;
//     send(
//         &mut write_half,
//         &Msg::Welcome {
//             text: format!(
//                 "Welcome {peer_name}, connected to {my_name} (id {}...)",
//                 &my_id[..8]
//             ),
//         },
//     )
//     .await?;
//     println!(
//         "[SERVER] handshake complete with {peer_name} ({})",
//         &peer_id[..8]
//     );

//     // --- REGISTER PEER & START WRITER ---
//     let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
//     {
//         let mut st = state.write().await;

//         // Insert peer
//         st.peers
//             .insert(peer_id.clone(), PeerEntry { name: peer_name.clone(), tx: tx.clone() });

//         // Send membership snapshot
//         let members = st
//             .peers
//             .iter()
//             .map(|(id, entry)| Member {
//                 node_id: id.clone(),
//                 name: entry.name.clone(),
//             })
//             .collect::<Vec<_>>();
//         let _ = tx.send(Msg::Peers { members });
//     }

//     // Dedicated writer
//     let writer = tokio::spawn(async move {
//         let mut wh = write_half;
//         while let Some(msg) = rx.recv().await {
//             if let Err(e) = crate::io::send(&mut wh, &msg).await {
//                 eprintln!("[SERVER] writer error: {e}");
//                 break;
//             }
//         }
//     });

//     // --- Reader: handle App + TxSubmit/TxAnnounce ---
//     let state_for_reader = Arc::clone(&state);
//     let peer_id_reader = peer_id.clone();
//     let reader_task = tokio::spawn(async move {
//         let mut rd = reader;
//         loop {
//             match crate::io::recv(&mut rd).await {
//                 Ok(Msg::App { from, text }) => {
//                     println!("[SERVER] msg from {}: {}", &from[..8], text);
//                     let st = state_for_reader.read().await;
//                     if let Some(entry) = st.peers.get(&peer_id_reader) {
//                         let _ = entry
//                             .tx
//                             .send(Msg::Ack { text: format!("received: {text}") });
//                     }
//                 }
//                 Ok(Msg::TxSubmit { tx }) => {
//                     let mut st = state_for_reader.write().await;
//                     let res = st.txs.insert_if_new(tx.clone());
//                     match res {
//                         Ok((id, inserted, _from)) => {
//                             println!(
//                                 "[SERVER] TxSubmit {} from {} accepted={} mempool={}",
//                                 &id[..8],
//                                 &peer_id_reader[..8],
//                                 inserted,
//                                 st.txs.by_id.len()
//                             );
//                             if let Some(entry) = st.peers.get(&peer_id_reader) {
//                                 let _ = entry.tx.send(Msg::TxAck {
//                                     id: id.clone(),
//                                     ok: true,
//                                     err: None,
//                                 });
//                             }
//                             if inserted {
//                                 // gossip to others
//                                 for (id_peer, entry) in st.peers.iter() {
//                                     if id_peer != &peer_id_reader {
//                                         let _ = entry.tx.send(Msg::TxAnnounce { tx: tx.clone() });
//                                     }
//                                 }
//                             }
//                         }
//                         Err(e) => {
//                             let id = crate::rpc::types::tx_id(&tx);
//                             println!(
//                                 "[SERVER] TxSubmit {} from {} rejected: {}",
//                                 &id[..8],
//                                 &peer_id_reader[..8],
//                                 e
//                             );
//                             if let Some(entry) = st.peers.get(&peer_id_reader) {
//                                 let _ = entry.tx.send(Msg::TxAck {
//                                     id,
//                                     ok: false,
//                                     err: Some(e.to_string()),
//                                 });
//                             }
//                         }
//                     }
//                 }
//                 Ok(Msg::TxAnnounce { tx }) => {
//                     let mut st = state_for_reader.write().await;
//                     match st.txs.insert_if_new(tx.clone()) {
//                         Ok((id, inserted, _)) => {
//                             println!(
//                                 "[SERVER] TxAnnounce {} new={} mempool={}",
//                                 &id[..8],
//                                 inserted,
//                                 st.txs.by_id.len()
//                             );
//                         }
//                         Err(_) => { /* ignore */ }
//                     }
//                 }
//                 Ok(other) => {
//                     println!(
//                         "[SERVER] unexpected from {}: {:?}",
//                         &peer_id_reader[..8], other
//                     );
//                 }
//                 Err(e) => {
//                     eprintln!("[SERVER] connection closed/err: {e}");
//                     break;
//                 }
//             }
//         }
//     });

//     let _ = reader_task.await;

//     // cleanup peer
//     {
//         let mut st = state.write().await;
//         st.peers.remove(&peer_id);
//         println!("[SERVER] peer ({}) disconnected", &peer_id[..8]);
//     }

//     drop(tx);
//     let _ = writer.await;

//     Ok(())
// }

use anyhow::{Context, Result};
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::Arc,
};
use tokio::{
    io::BufReader,
    net::{TcpListener, TcpStream},
    sync::{mpsc, RwLock},
};

use ed25519_dalek::{SigningKey, VerifyingKey, Signature};
use ed25519_dalek::Signer; // brings .sign(&[u8]) into scope
use rand::{rngs::OsRng, RngCore};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;

use axum::{
    extract::{Path, State as AxumState},
    routing::{get, post},
    Json, Router,
};

use crate::{
    consensus::clock::{SlotClock, now_secs, sleep_until, EPOCH_SLOTS},
    crypto::{gen_keypair, node_id, sign_nonce, verify_identity},
    io::{recv, send},
    protocol::{Msg, Member},
    rpc::mempool::TxStore,
    rpc::types::{SignedTx, tx_id, sign_tx, random_nonce},
    chain::types::{Block, BlockHeader, TxReceipt, header_hash, header_signing_bytes, list_root_hex},
};

// ============================ Node State ============================

struct PeerEntry {
    name: String,
    tx: mpsc::UnboundedSender<Msg>,
}

const BLOCK_BUFFER: usize = 256;

#[derive(Default)]
struct State {
    peers: HashMap<String, PeerEntry>, // node_id -> entry
    txs:   TxStore,
    // chain tip/finality
    tip_hash: String,
    tip_height: u64,
    finalized_height: u64,             // epoch-based finality
    // recent blocks for HTTP
    blocks: VecDeque<Block>,
}

// Context used by HTTP handlers
#[derive(Clone)]
struct HttpCtx {
    state: Arc<RwLock<State>>,
    sk: SigningKey,
    vk: VerifyingKey,
}

// ============================ Public API (HTTP) ============================

#[derive(serde::Deserialize)]
struct SubmitTxBody {
    // Option 1: a fully signed tx (wallet flow)
    tx: Option<SignedTx>,
    // Option 2: convenience path for quick testing – server signs a demo tx
    message: Option<String>,
}

#[derive(serde::Serialize)]
struct SubmitTxResp {
    id: String,
    ok: bool,
    err: Option<String>,
    inserted: bool,        // new to mempool?
    mempool_size: usize,   // mempool size after insert
    time_to_next_slot_ms: u64, // ms until next slot boundary
}

#[derive(serde::Serialize)]
struct MempoolEntry {
    id: String,
    from: String,
    nonce: u64,
    value: u128,
    max_fee: u64,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct BlockListEntry {
    height: u64,
    hash: String,
    slot: u64,
    epoch: u64,
    txs: usize,
    timestamp: u64,
}

#[derive(serde::Serialize)]
struct DebugState {
    tip_height: u64,
    finalized_height: u64,
    mempool_size: usize,
    // timing
    slot: u64,
    epoch: u64,
    next_slot_in_ms: u64,
}

// ---- NEW: per-tx lookup ----

#[derive(serde::Serialize)]
struct BlockRef {
    height: u64,
    hash: String,
    slot: u64,
    epoch: u64,
    timestamp: u64,
    finalized: bool,
}

#[derive(serde::Serialize)]
struct TxLookup {
    found: bool,
    location: String,                 // "mempool" | "block" | "none"
    tx: Option<SignedTx>,
    block: Option<BlockRef>,
    receipt: Option<TxReceipt>,
}

// POST /api/tx  (unchanged except for timing hint)
async fn post_tx(
    AxumState(ctx): AxumState<HttpCtx>,
    Json(body): Json<SubmitTxBody>,
) -> Json<SubmitTxResp> {
    // Build tx
    let maybe_tx = if let Some(tx) = body.tx {
        Some(tx)
    } else if let Some(msg) = body.message {
        Some(sign_tx(
            &ctx.sk,
            &ctx.vk,
            1,
            random_nonce(),
            None,
            0,
            1,
            base64::encode(msg.as_bytes()),
            600,
        ))
    } else {
        None
    };

    // timing hint
    let clock = SlotClock::new_aligned_now();
    let next = clock.next_slot_start();
    let now  = now_secs();
    let time_to_next_slot_ms = if next > now { (next - now) * 1000 } else { 0 };

    if maybe_tx.is_none() {
        return Json(SubmitTxResp {
            id: "".into(),
            ok: false,
            err: Some("provide `tx` or `message`".into()),
            inserted: false,
            mempool_size: 0,
            time_to_next_slot_ms,
        });
    }
    let tx = maybe_tx.unwrap();
    let id = tx_id(&tx);

    let mut inserted = false;
    let mut err_str = None;
    let mut to_gossip: Option<SignedTx> = None;
    let mut mempool_size = 0usize;

    {
        let mut st = ctx.state.write().await;
        match st.txs.insert_if_new(tx.clone()) {
            Ok((_id, ins, _from)) => {
                inserted = ins;
                mempool_size = st.txs.by_id.len();
                println!(
                    "[HTTP] TxSubmit {} accepted={} mempool={}",
                    &id[..8], inserted, mempool_size
                );
                if ins { to_gossip = Some(tx.clone()); }
            }
            Err(e) => {
                err_str = Some(e.to_string());
                mempool_size = st.txs.by_id.len();
                println!("[HTTP] TxSubmit {} rejected: {}", &id[..8], e);
            }
        }
        if let Some(tx_announce) = to_gossip {
            for (_peer, entry) in st.peers.iter() {
                let _ = entry.tx.send(Msg::TxAnnounce { tx: tx_announce.clone() });
            }
        }
    }

    Json(SubmitTxResp {
        id,
        ok: err_str.is_none(),
        err: err_str,
        inserted,
        mempool_size,
        time_to_next_slot_ms,
    })
}

async fn get_mempool(AxumState(ctx): AxumState<HttpCtx>) -> Json<Vec<MempoolEntry>> {
    let st = ctx.state.read().await;
    let mut out = Vec::new();
    for (id, tx) in st.txs.by_id.iter() {
        out.push(MempoolEntry {
            id: id.clone(),
            from: tx.from_pubkey_b64.clone(),
            nonce: tx.nonce,
            value: tx.value,
            max_fee: tx.max_fee,
            timestamp: tx.timestamp,
        });
    }
    Json(out)
}

async fn get_blocks(AxumState(ctx): AxumState<HttpCtx>) -> Json<Vec<BlockListEntry>> {
    let st = ctx.state.read().await;
    let mut out = Vec::new();
    for b in st.blocks.iter().rev() {
        out.push(BlockListEntry {
            height: b.header.height,
            hash: b.hash.clone(),
            slot: b.header.slot,
            epoch: b.header.epoch,
            txs: b.txs.len(),
            timestamp: b.header.timestamp,
        });
    }
    Json(out)
}

async fn get_block_by_height(
    AxumState(ctx): AxumState<HttpCtx>,
    Path(height): Path<u64>,
) -> Result<Json<Block>, axum::http::StatusCode> {
    let st = ctx.state.read().await;
    if let Some(b) = st.blocks.iter().find(|b| b.header.height == height) {
        return Ok(Json(b.clone()));
    }
    Err(axum::http::StatusCode::NOT_FOUND)
}

async fn get_debug(AxumState(ctx): AxumState<HttpCtx>) -> Json<DebugState> {
    let clock = SlotClock::new_aligned_now();
    let now = now_secs();
    let next = clock.next_slot_start();
    let slot = clock.now_slot();
    let epoch = clock.epoch_of_slot(slot);

    let st = ctx.state.read().await;
    Json(DebugState {
        tip_height: st.tip_height,
        finalized_height: st.finalized_height,
        mempool_size: st.txs.by_id.len(),
        slot,
        epoch,
        next_slot_in_ms: if next > now { (next - now) * 1000 } else { 0 },
    })
}

// ---- NEW: /api/tx/:id -> look up in mempool or blocks ----
async fn get_tx_by_id(
    AxumState(ctx): AxumState<HttpCtx>,
    Path(id): Path<String>,
) -> Json<TxLookup> {
    let st = ctx.state.read().await;

    // 1) mempool
    if let Some(tx) = st.txs.by_id.get(&id) {
        return Json(TxLookup {
            found: true,
            location: "mempool".into(),
            tx: Some(tx.clone()),
            block: None,
            receipt: None,
        });
    }

    // 2) recent blocks (search newest first)
    for b in st.blocks.iter().rev() {
        // receipts align with tx order in our producer
        for (i, r) in b.receipts.iter().enumerate() {
            if r.tx_id == id {
                let finalized = b.header.height <= st.finalized_height;
                let bref = BlockRef {
                    height: b.header.height,
                    hash: b.hash.clone(),
                    slot: b.header.slot,
                    epoch: b.header.epoch,
                    timestamp: b.header.timestamp,
                    finalized,
                };
                // get tx body (same index)
                let tx_opt = b.txs.get(i).cloned()
                    .or_else(|| b.txs.iter().find(|t| crate::rpc::types::tx_id(t) == id).cloned());
                return Json(TxLookup {
                    found: true,
                    location: "block".into(),
                    tx: tx_opt,
                    block: Some(bref),
                    receipt: Some(r.clone()),
                });
            }
        }
    }

    // 3) not found
    Json(TxLookup {
        found: false,
        location: "none".into(),
        tx: None,
        block: None,
        receipt: None,
    })
}

// ---- NEW: /api/receipt/:id -> only the receipt (404 if not found) ----
async fn get_receipt_by_id(
    AxumState(ctx): AxumState<HttpCtx>,
    Path(id): Path<String>,
) -> Result<Json<TxReceipt>, axum::http::StatusCode> {
    let st = ctx.state.read().await;
    for b in st.blocks.iter().rev() {
        if let Some((_, r)) = b.receipts.iter().enumerate().find(|(_, r)| r.tx_id == id) {
            return Ok(Json(r.clone()));
        }
    }
    Err(axum::http::StatusCode::NOT_FOUND)
}

fn spawn_http(ctx: HttpCtx, addr: &str) {
    let app = Router::new()
        .route("/api/tx", post(post_tx))
        .route("/api/mempool", get(get_mempool))
        .route("/api/blocks", get(get_blocks))
        .route("/api/blocks/height/:height", get(get_block_by_height))
        .route("/api/debug", get(get_debug))
        // NEW
        .route("/api/tx/:id", get(get_tx_by_id))
        .route("/api/receipt/:id", get(get_receipt_by_id))
        .with_state(ctx);

    let addr: SocketAddr = addr.parse().expect("bad --http-addr");
    tokio::spawn(async move {
        println!("[HTTP] listening on http://{}", addr);
        if let Err(e) = axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app).await {
            eprintln!("[HTTP] server error: {e}");
        }
    });
}

// ============================ TCP Node (server) ============================

pub async fn run_server(port: u16, name: String) -> Result<()> {
    let (my_sk, my_vk, _seed) = gen_keypair("NODE1 (server)");
    let my_id = node_id(&my_vk);

    let mut my_name = name;
    if my_name == "auto" { my_name = format!("node-{}", &my_id[..8]); }

    let bind: SocketAddr = format!("0.0.0.0:{port}").parse().unwrap();
    println!("[SERVER] listening on {bind}  id={}  name={}", &my_id[..8], my_name);

    let listener = TcpListener::bind(bind).await?;
    let state = Arc::new(RwLock::new(State {
        peers: HashMap::new(),
        txs: TxStore::default(),
        tip_hash: "GENESIS".into(),
        tip_height: 0,
        finalized_height: 0, // epoch-based
        blocks: VecDeque::with_capacity(BLOCK_BUFFER),
    }));

    // HTTP API
    spawn_http(
        HttpCtx { state: Arc::clone(&state), sk: my_sk.clone(), vk: my_vk.clone() },
        "127.0.0.1:9000",
    );

    // Block producer
    {
        let state_cloned = Arc::clone(&state);
        let my_sk = my_sk.clone();
        let my_vk = my_vk.clone();
        tokio::spawn(async move { run_block_producer(state_cloned, my_sk, my_vk).await; });
    }

    loop {
        let (stream, peer) = listener.accept().await?;
        println!("[SERVER] incoming from {peer}");

        let my_sk = my_sk.clone();
        let my_vk = my_vk.clone();
        let my_id = my_id.clone();
        let my_name_cloned = my_name.clone();
        let state_cloned = Arc::clone(&state);

        tokio::spawn(async move {
            if let Err(e) = handle_server_conn(stream, my_name_cloned, my_sk, my_vk, my_id, state_cloned).await {
                eprintln!("[SERVER] error: {e:?}");
            }
        });
    }
}

async fn run_block_producer(state: Arc<RwLock<State>>, sk: SigningKey, vk: VerifyingKey) {
    const MAX_TXS_PER_BLOCK: usize = 100;
    let clock = SlotClock::new_aligned_now();
    loop {
        let next = clock.next_slot_start();
        sleep_until(next).await;

        let slot = clock.now_slot();
        let epoch = clock.epoch_of_slot(slot);

        // drain txs
        let (parent_hash, height, txs) = {
            let mut st = state.write().await;
            let txs = st.txs.drain_block(MAX_TXS_PER_BLOCK);
            let h = st.tip_height + 1;
            let ph = st.tip_hash.clone();
            (ph, h, txs)
        };

        // receipts & roots
        let receipts: Vec<TxReceipt> = txs.iter().enumerate().map(|(i, tx)| TxReceipt {
            tx_id: tx_id(tx),
            status: true,
            tx_index: i as u32,
            gas_used: 21_000,
            fee_paid: tx.max_fee,
        }).collect();

        let tx_ids: Vec<String> = receipts.iter().map(|r| r.tx_id.clone()).collect();
        let txs_root = list_root_hex(&tx_ids);
        let receipts_root = list_root_hex(&tx_ids);

        let header = BlockHeader {
            height,
            slot,
            epoch,
            parent_hash,
            txs_root,
            receipts_root,
            timestamp: now_secs(),
            proposer_pubkey_b64: B64.encode(vk.as_bytes()),
            signature_b64: String::new(),
        };

        // sign header
        let signing_bytes = header_signing_bytes(&header);
        let sig = sk.sign(&signing_bytes);
        let mut header = header;
        header.signature_b64 = B64.encode(sig.to_bytes());

        let hash = header_hash(&header);
        let block = Block { header: header.clone(), txs: txs.clone(), receipts: receipts.clone(), hash: hash.clone() };

        // update tip & epoch-based finality, store block
        let finalized_h = {
            let mut st = state.write().await;
            st.tip_hash = hash.clone();
            st.tip_height = height;
            st.blocks.push_back(block.clone());
            if st.blocks.len() > BLOCK_BUFFER { st.blocks.pop_front(); }

            // finalize at epoch boundaries: finalize last block of (epoch - 2)
            let slot_in_epoch = slot % EPOCH_SLOTS;
            if slot_in_epoch == 0 {
                if epoch >= 2 {
                    st.finalized_height = (epoch - 2 + 1) * EPOCH_SLOTS;
                } else {
                    st.finalized_height = 0;
                }
            }
            st.finalized_height
        };

        println!(
            "[PROPOSER] block h={} slot={} epoch={} txs={} finalized_h={}",
            height, slot, epoch, block.txs.len(), finalized_h
        );

        // broadcast to peers
        let peers = {
            let st = state.read().await;
            st.peers.values().map(|p| p.tx.clone()).collect::<Vec<_>>()
        };
        for tx in peers {
            let _ = tx.send(Msg::NewBlock { block: block.clone() });
        }
    }
}

async fn handle_server_conn(
    stream: TcpStream,
    my_name: String,
    my_sk: SigningKey,
    my_vk: VerifyingKey,
    my_id: String,
    state: Arc<RwLock<State>>,
) -> Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    // --- HANDSHAKE ---
    let mut nonce = [0u8; 32];
    OsRng.fill_bytes(&mut nonce);
    println!("[SERVER] sending Challenge nonce={}", hex::encode(nonce));
    send(&mut write_half, &Msg::Challenge { nonce_b64: B64.encode(nonce) }).await?;

    let (peer_name, peer_id) = match recv(&mut reader).await? {
        Msg::Identity { name, node_id, pubkey_b64, sig_b64 } => {
            println!("[SERVER] verifying client Identity: name={name}, node_id={}...", &node_id[..8]);
            verify_identity(&nonce, &node_id, &pubkey_b64, &sig_b64)
                .context("client identity verification failed")?;
            println!("[SERVER] ✅ client authed: name={name} id={}", &node_id[..8]);
            (name, node_id)
        }
        other => anyhow::bail!("expected Identity, got {other:?}"),
    };

    let sig_b64 = sign_nonce(&my_sk, &nonce);
    send(&mut write_half, &Msg::Identity {
        name: my_name.clone(),
        node_id: my_id.clone(),
        pubkey_b64: B64.encode(my_vk.as_bytes()),
        sig_b64,
    }).await?;
    send(&mut write_half, &Msg::Welcome {
        text: format!("Welcome {peer_name}, connected to {my_name} (id {}...)", &my_id[..8]),
    }).await?;
    println!("[SERVER] handshake complete with {peer_name} ({})", &peer_id[..8]);

    // --- REGISTER PEER & WRITER ---
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
    {
        let mut st = state.write().await;
        st.peers.insert(peer_id.clone(), PeerEntry { name: peer_name.clone(), tx: tx.clone() });

        let members = st.peers.iter()
            .map(|(id, entry)| Member { node_id: id.clone(), name: entry.name.clone() })
            .collect::<Vec<_>>();
        let _ = tx.send(Msg::Peers { members });
    }

    let writer = tokio::spawn(async move {
        let mut wh = write_half;
        while let Some(msg) = rx.recv().await {
            if let Err(e) = crate::io::send(&mut wh, &msg).await {
                eprintln!("[SERVER] writer error: {e}");
                break;
            }
        }
    });

    // --- Reader: App + TxSubmit/TxAnnounce ---
    let state_for_reader = Arc::clone(&state);
    let peer_id_reader = peer_id.clone();
    let reader_task = tokio::spawn(async move {
        let mut rd = reader;
        loop {
            match crate::io::recv(&mut rd).await {
                Ok(Msg::App { from, text }) => {
                    println!("[SERVER] msg from {}: {}", &from[..8], text);
                    let st = state_for_reader.read().await;
                    if let Some(entry) = st.peers.get(&peer_id_reader) {
                        let _ = entry.tx.send(Msg::Ack { text: format!("received: {text}") });
                    }
                }
                Ok(Msg::TxSubmit { tx }) => {
                    let mut st = state_for_reader.write().await;
                    let res = st.txs.insert_if_new(tx.clone());
                    match res {
                        Ok((id, inserted, _from)) => {
                            println!("[SERVER] TxSubmit {} from {} accepted={} mempool={}", &id[..8], &peer_id_reader[..8], inserted, st.txs.by_id.len());
                            if let Some(entry) = st.peers.get(&peer_id_reader) {
                                let _ = entry.tx.send(Msg::TxAck { id: id.clone(), ok: true, err: None });
                            }
                            if inserted {
                                for (id_peer, entry) in st.peers.iter() {
                                    if id_peer != &peer_id_reader {
                                        let _ = entry.tx.send(Msg::TxAnnounce { tx: tx.clone() });
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let id = crate::rpc::types::tx_id(&tx);
                            println!("[SERVER] TxSubmit {} from {} rejected: {}", &id[..8], &peer_id_reader[..8], e);
                            if let Some(entry) = st.peers.get(&peer_id_reader) {
                                let _ = entry.tx.send(Msg::TxAck { id, ok: false, err: Some(e.to_string()) });
                            }
                        }
                    }
                }
                Ok(Msg::TxAnnounce { tx }) => {
                    let mut st = state_for_reader.write().await;
                    match st.txs.insert_if_new(tx.clone()) {
                        Ok((id, inserted, _)) => {
                            println!("[SERVER] TxAnnounce {} new={} mempool={}", &id[..8], inserted, st.txs.by_id.len());
                        }
                        Err(_) => { /* ignore */ }
                    }
                }
                Ok(other) => {
                    println!("[SERVER] unexpected from {}: {:?}", &peer_id_reader[..8], other);
                }
                Err(e) => {
                    eprintln!("[SERVER] connection closed/err: {e}");
                    break;
                }
            }
        }
    });

    let _ = reader_task.await;

    // cleanup
    {
        let mut st = state.write().await;
        st.peers.remove(&peer_id);
        println!("[SERVER] peer ({}) disconnected", &peer_id[..8]);
    }

    drop(tx);
    let _ = writer.await;

    Ok(())
}
