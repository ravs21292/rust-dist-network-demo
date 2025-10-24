// use anyhow::{Context, Result};
// use std::net::SocketAddr;
// use tokio::{
//     io::{BufReader, AsyncBufReadExt},   // <-- bring AsyncBufReadExt for .lines()
//     net::TcpStream,
// };

// use ed25519_dalek::VerifyingKey;

// use base64::engine::general_purpose::STANDARD as B64;
// use base64::Engine as _;

// use crate::{
//     crypto::{gen_keypair, node_id, sign_nonce, verify_identity},
//     io::{recv, send},
//     protocol::Msg,
// };

// pub async fn run_client(addr: String, port: u16, name: String) -> Result<()> {
//     let (my_sk, my_vk, _seed) = gen_keypair("Node (Client)");
//     let my_id = node_id(&my_vk);
//     let mut my_name = name;
//     if my_name == "auto" {
//         my_name = format!("node-{}", &my_id[..8]);
//     }

//     let target: SocketAddr = format!("{addr}:{port}").parse().context("bad address")?;
//     println!("[CLIENT] connecting to {target} with id={}", &my_id[..8]);
//     let stream = TcpStream::connect(target).await?;
//     let local = stream.local_addr()?;
//     let peer  = stream.peer_addr()?;
//     println!("[CLIENT] socket {} -> {}", local, peer);
//     println!("[CLIENT] connected, starting handshake…");

//     let (read_half, mut write_half) = stream.into_split();
//     let mut reader = BufReader::new(read_half);

//     // --- HANDSHAKE ---
//     // 1) expect Challenge
//     let nonce = match recv(&mut reader).await? {
//         Msg::Challenge { nonce_b64 } => {
//             let nonce = B64.decode(nonce_b64.as_bytes())?;
//             println!("[CLIENT] got Challenge (nonce={}, len={})", hex::encode(&nonce), nonce.len());
//             nonce
//         }
//         other => anyhow::bail!("expected Challenge, got {other:?}"),
//     };

//     // 2) send our Identity (sign server's nonce)
//     let sig_b64 = sign_nonce(&my_sk, &nonce);
//     let sig_short = format!("{}...", &sig_b64[..24]); // pretty log
//     send(&mut write_half, &Msg::Identity {
//         name: my_name.clone(),
//         node_id: my_id.clone(),
//         pubkey_b64: B64.encode(my_vk.as_bytes()),
//         sig_b64: sig_b64.clone(),
//     }).await?;
//     println!(
//         "[CLIENT] sent Identity: name={my_name}, my_id={}, pubkey={}, sig={}",
//         &my_id[..8],
//         hex::encode(my_vk.as_bytes()),
//         sig_short
//     );

//     // 3) receive server Identity and Welcome
//     let mut server_name = String::new();
//     match recv(&mut reader).await? {
//         Msg::Identity { name: srv_name, node_id: srv_node_id, pubkey_b64, sig_b64 } => {
//             // verbose verification logs on the client
//             println!("[CLIENT] verifying server Identity:");
//             println!("          name         : {srv_name}");
//             println!("          node_id(sent): {}...", &srv_node_id[..8]);

//             // decode pubkey and compute node_id for comparison
//             let pk_bytes = B64.decode(pubkey_b64.as_bytes())?;
//             println!("          pubkey(hex)  : {}", hex::encode(&pk_bytes));
//             let pk_arr: [u8; 32] = pk_bytes.as_slice().try_into().map_err(|_| anyhow::anyhow!("bad pubkey len"))?;
//             let vk = VerifyingKey::from_bytes(&pk_arr)?;
//             let expect_node_id = node_id(&vk);
//             println!("          node_id(calc): {}...", &expect_node_id[..8]);
//             if expect_node_id == srv_node_id {
//                 println!("[CLIENT]   node_id check ✅ matches");
//             } else {
//                 println!("[CLIENT]   node_id check ❌ mismatch");
//             }

//             // show signature then verify strictly
//             let sig_bytes = B64.decode(sig_b64.as_bytes())?;
//             println!("          sig(hex)     : {}", hex::encode(&sig_bytes));
//             verify_identity(&nonce, &srv_node_id, &pubkey_b64, &sig_b64)
//                 .context("server identity verification failed")?;
//             println!("[CLIENT]   signature verify ✅ OK");

//             server_name = srv_name;
//         }
//         other => anyhow::bail!("expected server Identity, got {other:?}"),
//     }

//     if let Msg::Welcome { text } = recv(&mut reader).await? {
//         println!("[CLIENT] received Welcome from {server_name}: {text}");
//     }
//     println!("[CLIENT] handshake complete ✅\n");

//     // --- APP: type lines -> send; print ACKs ---
//     let mut stdin_lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
//     loop {
//         tokio::select! {
//             // type a line and press Enter to send
//             line = stdin_lines.next_line() => {
//                 match line? {
//                     Some(text) if !text.is_empty() => {
//                         send(&mut write_half, &Msg::App { from: my_id.clone(), text }).await?;
//                     }
//                     Some(_) => {} // ignore empty lines
//                     None => break, // stdin closed
//                 }
//             }
//             incoming = recv(&mut reader) => {
//                 match incoming {
//                     Ok(Msg::Ack { text }) => println!("[CLIENT] ACK: {}", text),
//                     Ok(other) => println!("[CLIENT] from server: {:?}", other),
//                     Err(e) => { eprintln!("[CLIENT] server closed/err: {e}"); break; }
//                 }
//             }
//         }
//     }
//     Ok(())
// }

use anyhow::{Context, Result};
use std::{collections::HashMap, net::SocketAddr};
use tokio::{
    io::{BufReader, AsyncBufReadExt},
    net::TcpStream,
};
use ed25519_dalek::{VerifyingKey, Signature};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;

use crate::{
    consensus::clock::{SlotClock, EPOCH_SLOTS},
    crypto::{gen_keypair, node_id, sign_nonce, verify_identity},
    io::{recv, send},
    protocol::{Msg, Member},
    rpc::{mempool::TxStore, types::{sign_tx, random_nonce}},
    chain::types::{Block, header_hash, header_signing_bytes},
};

pub async fn run_client(addr: String, port: u16, name: String) -> Result<()> {
    let (my_sk, my_vk, _seed) = gen_keypair("NODE (client)");
    let my_id = node_id(&my_vk);

    let mut my_name = name;
    if my_name == "auto" { my_name = format!("node-{}", &my_id[..8]); }

    let target: SocketAddr = format!("{addr}:{port}").parse().context("bad address")?;
    println!("[CLIENT] connecting to {target} with id={} name={}", &my_id[..8], my_name);
    let stream = TcpStream::connect(target).await?;
    let local = stream.local_addr()?;
    let peer  = stream.peer_addr()?;
    println!("[CLIENT] socket {} -> {}", local, peer);
    println!("[CLIENT] connected, starting handshake…");

    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    // --- HANDSHAKE ---
    let nonce = match recv(&mut reader).await? {
        Msg::Challenge { nonce_b64 } => {
            let nonce = B64.decode(nonce_b64.as_bytes())?;
            println!("[CLIENT] got Challenge (nonce={}, len={})", hex::encode(&nonce), nonce.len());
            nonce
        }
        other => anyhow::bail!("expected Challenge, got {other:?}"),
    };

    let sig_b64 = sign_nonce(&my_sk, &nonce);
    send(&mut write_half, &Msg::Identity {
        name: my_name.clone(),
        node_id: my_id.clone(),
        pubkey_b64: B64.encode(my_vk.as_bytes()),
        sig_b64: sig_b64.clone(),
    }).await?;
    println!(
        "[CLIENT] sent Identity: name={}, my_id={}, pubkey={}, sig={}...",
        my_name, &my_id[..8], hex::encode(my_vk.as_bytes()), &sig_b64[..24]
    );

    let mut server_name = String::new();
    let mut server_vk   = None::<VerifyingKey>;
    match recv(&mut reader).await? {
        Msg::Identity { name: srv_name, node_id: srv_node_id, pubkey_b64, sig_b64 } => {
            println!("[CLIENT] verifying server Identity: {}", srv_name);
            let pk_bytes = B64.decode(pubkey_b64.as_bytes())?;
            let pk_arr: [u8;32] = pk_bytes.as_slice().try_into().map_err(|_| anyhow::anyhow!("bad pubkey len"))?;
            let vk = VerifyingKey::from_bytes(&pk_arr)?;
            let expect_node_id = node_id(&vk);
            verify_identity(&nonce, &srv_node_id, &pubkey_b64, &sig_b64)
                .context("server identity verification failed")?;
            println!("[CLIENT]   signature verify ✅ OK");
            server_name = srv_name;
            server_vk = Some(vk);
        }
        other => anyhow::bail!("expected server Identity, got {other:?}"),
    }
    if let Msg::Welcome { text } = recv(&mut reader).await? {
        println!("[CLIENT] received Welcome from {server_name}: {text}");
    }
    println!("[CLIENT] handshake complete ✅\n");

    // local mempool + chain view
    let mut mempool = TxStore::default();
    let mut tip_height: u64 = 0;
    let mut finalized_height: u64 = 0;
    let clock = SlotClock::new_aligned_now();

    println!("Type `/tx your-message` to submit a transaction; plain text still sends App messages.");
    let mut stdin_lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();

    loop {
        tokio::select! {
            line = stdin_lines.next_line() => {
                match line? {
                    Some(l) if l.starts_with("/tx ") => {
                        let payload = l.trim_start_matches("/tx ").to_string();
                        let tx = sign_tx(&my_sk, &my_vk, 1, random_nonce(), None, 0, 1, base64::encode(payload.as_bytes()), 600);
                        let id = crate::rpc::types::tx_id(&tx);
                        send(&mut write_half, &Msg::TxSubmit { tx }).await?;
                        println!("[CLIENT] TxSubmit sent id={}", &id[..8]);
                    }
                    Some(text) if !text.is_empty() => {
                        send(&mut write_half, &Msg::App { from: my_id.clone(), text }).await?;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            incoming = recv(&mut reader) => {
                match incoming {
                    Ok(Msg::Ack { text }) => println!("[CLIENT] ACK: {}", text),
                    Ok(Msg::Peers { members }) => {
                        println!("[CLIENT] peers snapshot ({}):", members.len());
                        for m in members { println!("  - {} ({})", m.name, &m.node_id[..8]); }
                    }
                    Ok(Msg::TxAck { id, ok, err }) => {
                        println!("[CLIENT] TxAck id={} ok={} err={:?}", &id[..8], ok, err);
                    }
                    Ok(Msg::TxAnnounce { tx }) => {
                        let _ = mempool.insert_if_new(tx);
                        println!("[CLIENT] mempool size = {}", mempool.by_id.len());
                    }
                    Ok(Msg::NewBlock { block }) => {
                        on_new_block(block, &mut mempool, &mut tip_height, &mut finalized_height, &server_vk, &clock).await?;
                    }
                    Ok(other) => println!("[CLIENT] from server: {:?}", other),
                    Err(e) => { eprintln!("[CLIENT] server closed/err: {e}"); break; }
                }
            }
        }
    }
    Ok(())
}

async fn on_new_block(
    block: Block,
    mempool: &mut TxStore,
    tip_height: &mut u64,
    finalized_height: &mut u64,
    server_vk: &Option<VerifyingKey>,
    clock: &crate::consensus::clock::SlotClock,
) -> Result<()> {
    // verify header hash & signature
    let h = &block.header;
    let hash = header_hash(h);
    anyhow::ensure!(hash == block.hash, "bad block hash");

    if let Some(vk) = server_vk {
        let sig_bytes = B64.decode(h.signature_b64.as_bytes())?;
        let sig_arr: [u8;64] = sig_bytes.as_slice().try_into().map_err(|_| anyhow::anyhow!("bad sig len"))?;
        let sig = Signature::from_bytes(&sig_arr);
        let signing_bytes = header_signing_bytes(h);
        vk.verify_strict(&signing_bytes, &sig).map_err(|e| anyhow::anyhow!("header sig verify failed: {e}"))?;
    }

    // prune included txs
    for tx in &block.txs {
        let id = crate::rpc::types::tx_id(tx);
        mempool.by_id.remove(&id);
    }

    *tip_height = block.header.height;

    // NEW: epoch-based finality (step at epoch boundaries)
    let slot = block.header.slot;
    let epoch = block.header.epoch;
    let slot_in_epoch = slot % EPOCH_SLOTS;
    if slot_in_epoch == 0 {
        if epoch >= 2 {
            *finalized_height = (epoch - 2 + 1) * EPOCH_SLOTS;
        } else {
            *finalized_height = 0;
        }
    }

    println!(
        "[CLIENT] NEW BLOCK h={} slot={} epoch={} txs={} mempool={} finalized_h={}",
        block.header.height, block.header.slot, block.header.epoch, block.txs.len(), mempool.by_id.len(), *finalized_height
    );
    Ok(())
}


