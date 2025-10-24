// use axum::{routing::get, Json, Router};
// use axum::serve;
// use serde::Serialize;
// use std::net::SocketAddr;
// use tokio::net::TcpListener;

// #[derive(Serialize)]
// struct Message {
//     message: String,
// }

// #[derive(Serialize)]
// struct User {
//     id: u32,
//     name: String,
//     email: String,
// }

// async fn hello() -> Json<Message> {
//     Json(Message {
//         message: "Hello from Axum!".to_string(),
//     })
// }

// async fn get_user() -> Json<User> {
//     Json(User {
//         id: 1,
//         name: "J".to_string(),
//         email: "j@example.com".to_string(),
//     })
// }

// #[tokio::main]
// async fn main() {
//     let app = Router::new()
//         .route("/", get(hello))
//         .route("/user", get(get_user));

//     let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
//     let listener = TcpListener::bind(addr).await.unwrap();

//     println!("Server running at http://{}", addr);

//     serve(listener, app).await.unwrap();
// }

// use ed25519_dalek::{SigningKey, VerifyingKey, Signature};
// use ed25519_dalek::Signer; // brings `.sign()` into scope
// use rand::{rngs::OsRng, RngCore};

// fn main() {
//     // 1) Random 32-byte seed (private key material)
//     let mut seed = [0u8; 32];
//     OsRng.fill_bytes(&mut seed);

//     // 2) Build keys FROM THE SEED (no feature flags, no generate())
//     let signing_key = SigningKey::from_bytes(&seed);
//     let verifying_key = VerifyingKey::from(&signing_key);

//     // 3) Sign
//     let msg = b"hello from ed25519";
//     let sig: Signature = signing_key.sign(msg);

//     // 4) Verify
//     println!("private seed: {}", hex::encode(seed));
//     println!("public key  : {}", hex::encode(verifying_key.as_bytes()));
//     println!("signature   : {}", hex::encode(sig.to_bytes()));

//     match verifying_key.verify_strict(msg, &sig) {
//         Ok(()) => println!("✅ verify OK"),
//         Err(e) => println!("❌ verify failed: {e}"),
//     }

//     // 5) Tamper test
//     let bad = b"hello from ed2551X";
//     println!(
//         "tampered verify: {}",
//         if verifying_key.verify_strict(bad, &sig).is_ok() {
//             "UNEXPECTED OK"
//         } else {
//             "failed (as expected)"
//         }
//     );
// }


// use ed25519_dalek::{SigningKey, VerifyingKey, Signature};
// use ed25519_dalek::Signer; // brings `.sign()` into scope
// use rand::{rngs::OsRng, RngCore};

// fn gen_keypair(label: &str) -> (SigningKey, VerifyingKey, [u8; 32]) {
//     // Generate a random 32-byte seed (private key material)
//     let mut seed = [0u8; 32];
//     OsRng.fill_bytes(&mut seed);

//     // Build SigningKey from seed; derive VerifyingKey
//     let sk = SigningKey::from_bytes(&seed);
//     let vk = VerifyingKey::from(&sk);

//     println!("--- {label} KEYPAIR ---");
//     println!("private seed (hex): {}", hex::encode(seed));
//     println!("public key   (hex): {}", hex::encode(vk.as_bytes()));
//     println!();

//     (sk, vk, seed)
// }

// fn main() {
//     // STEP 0: Define the message Alice will sign
//     let message = b"hello from ed25519 (real flow: sender signs, receiver verifies)";

//     // STEP 1: Alice generates keys (sender)
//     let (alice_sk, alice_vk, _alice_seed) = gen_keypair("ALICE (sender)");

//     // STEP 2: Bob generates keys (receiver) — independent keys
//     let (_bob_sk, bob_vk, _bob_seed) = gen_keypair("BOB (receiver)");

//     // STEP 3: Alice signs the message with her PRIVATE key
//     println!("--- SIGNING ---");
//     let sig: Signature = alice_sk.sign(message);
//     let sig_hex = hex::encode(sig.to_bytes());
//     println!("message (utf8): {}", String::from_utf8_lossy(message));
//     println!("signature (hex): {sig_hex}");
//     println!("(Alice will send: message + signature + Alice's PUBLIC key)\n");

//     // Simulate “network transfer” to Bob:
//     let transmitted_msg = message;                 // bytes
//     let transmitted_sig = sig;                     // 64-byte signature
//     let transmitted_alice_pub = alice_vk;          // 32-byte public key

//     // STEP 4: Bob verifies using ALICE'S PUBLIC KEY (correct)
//     println!("--- VERIFY (correct public key: Alice's) ---");
//     match transmitted_alice_pub.verify_strict(transmitted_msg, &transmitted_sig) {
//         Ok(()) => println!("✅ verification OK (Bob verified using Alice's public key)"),
//         Err(e) => println!("❌ verification FAILED (unexpected): {e}"),
//     }
//     println!();

//     // STEP 5: Show that verification FAILS with the WRONG public key (Bob's)
//     println!("--- VERIFY (wrong public key: Bob's) ---");
//     match bob_vk.verify_strict(transmitted_msg, &transmitted_sig) {
//         Ok(()) => println!("❌ UNEXPECTED: verified with Bob's public key"),
//         Err(e) => println!("👍 expected failure with Bob's public key: {e}"),
//     }
//     println!();

//     // STEP 6: Tamper test — Bob receives message/signature but message is altered
//     println!("--- TAMPER TEST ---");
//     let tampered = b"hello from ed25519 (tampered)";
//     match transmitted_alice_pub.verify_strict(tampered, &transmitted_sig) {
//         Ok(()) => println!("❌ UNEXPECTED: tampered message verified"),
//         Err(e) => println!("👍 expected failure on tampered message: {e}"),
//     }
// }
// use anyhow::{Context, Result};
// use clap::{Parser, Subcommand};
// use ed25519_dalek::{SigningKey, VerifyingKey, Signature};
// use ed25519_dalek::Signer; // for .sign()
// use rand::{rngs::OsRng, RngCore};
// use sha2::{Digest, Sha256};
// use std::net::SocketAddr;
// use tokio::{
//     io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
//     net::{TcpListener, TcpStream},
// };

// // Base64 engine (non-deprecated API)
// use base64::engine::general_purpose::STANDARD as B64;
// use base64::Engine as _;

// #[derive(Parser, Debug)]
// #[command(name="tcp-handshake-demo", about="TCP handshake + signed identity + message transfer")]
// struct Args {
//     #[command(subcommand)]
//     cmd: Cmd,
// }

// #[derive(Subcommand, Debug)]
// enum Cmd {
//     /// Run Node1: listen on a port
//     Listen {
//         #[arg(long, default_value_t = 7000)]
//         port: u16,
//         #[arg(long, default_value = "NODE1")]
//         name: String,
//     },
//     /// Run Node2: connect to Node1 at addr:port
//     Connect {
//         #[arg(long)]
//         addr: String,
//         #[arg(long)]
//         port: u16,
//         #[arg(long, default_value = "NODE2")]
//         name: String,
//     },
// }
//  // Implemented 
// #[derive(Debug, serde::Serialize, serde::Deserialize)]
// #[serde(tag = "type", content = "data")]
// enum Msg {
//     // handshake
//     Challenge { nonce_b64: String },
//     Identity { name: String, node_id: String, pubkey_b64: String, sig_b64: String },
//     Welcome { text: String },

//     // app messages
//     App { from: String, text: String },
//     Ack { text: String },
// }

// fn gen_keypair(label: &str) -> (SigningKey, VerifyingKey, [u8; 32]) {
//     let mut seed = [0u8; 32];
//     OsRng.fill_bytes(&mut seed);
//     let sk = SigningKey::from_bytes(&seed);
//     let vk = VerifyingKey::from(&sk);

//     println!("--- {label} KEYPAIR ---");
//     println!("private seed (hex): {}", hex::encode(seed));
//     println!("public key   (hex): {}", hex::encode(vk.as_bytes()));
//     println!();

//     (sk, vk, seed)
// }

// fn node_id(vk: &VerifyingKey) -> String {
//     let mut h = Sha256::new();
//     h.update(vk.as_bytes());
//     hex::encode(h.finalize())
// }

// fn sign_nonce(sk: &SigningKey, nonce: &[u8]) -> String {
//     let sig = sk.sign(nonce);
//     B64.encode(sig.to_bytes())
// }

// fn verify_identity(nonce: &[u8], node_id_hex: &str, pubkey_b64: &str, sig_b64: &str) -> Result<()> {
//     // decode peer public key (base64) -> VerifyingKey
//     let pk_bytes = B64.decode(pubkey_b64.as_bytes())?;
//     let pk_arr: [u8; 32] = pk_bytes.as_slice().try_into().map_err(|_| anyhow::anyhow!("bad pubkey len"))?;
//     let vk = VerifyingKey::from_bytes(&pk_arr)?;

//     // node_id must be sha256(pubkey)
//     let expect = {
//         let mut h = Sha256::new();
//         h.update(vk.as_bytes());
//         hex::encode(h.finalize())
//     };
//     anyhow::ensure!(expect == node_id_hex, "node_id mismatch");

//     // decode signature (base64) -> Signature
//     let sig_bytes = B64.decode(sig_b64.as_bytes())?;
//     let sig_arr: [u8; 64] = sig_bytes.as_slice().try_into().map_err(|_| anyhow::anyhow!("bad sig len"))?;
//     let sig = Signature::from_bytes(&sig_arr);

//     // verify
//     vk.verify_strict(nonce, &sig).map_err(|e| anyhow::anyhow!("verify failed: {e}"))?;
//     Ok(())
// }

// #[tokio::main]
// async fn main() -> Result<()> {
//     let args = Args::parse();

//     match args.cmd {
//         Cmd::Listen { port, name } => run_server(port, name).await,
//         Cmd::Connect { addr, port, name } => run_client(addr, port, name).await,
//     }
// }

// async fn run_server(port: u16, name: String) -> Result<()> {
//     // Node1 keys
//     let (my_sk, my_vk, _seed) = gen_keypair("NODE1 (server)");
//     let my_id = node_id(&my_vk);

//     let bind: SocketAddr = format!("0.0.0.0:{port}").parse().unwrap();
//     println!("[SERVER] listening on {bind}  id={}", &my_id[..8]);

//     let listener = TcpListener::bind(bind).await?;
//     loop {
//         let (stream, peer) = listener.accept().await?;
//         println!("[SERVER] incoming from {peer}");

//         let my_sk = my_sk.clone();
//         let my_vk = my_vk.clone();
//         let my_id = my_id.clone();
//         let name = name.clone();

//         tokio::spawn(async move {
//             if let Err(e) = handle_server_conn(stream, name, my_sk, my_vk, my_id).await {
//                 eprintln!("[SERVER] error: {e:?}");
//             }
//         });
//     }
// }

// async fn handle_server_conn(
//     stream: TcpStream,
//     my_name: String,
//     my_sk: SigningKey,
//     my_vk: VerifyingKey,
//     my_id: String,
// ) -> Result<()> {
//     let (read_half, mut write_half) = stream.into_split();
//     let mut reader = BufReader::new(read_half);

//     // --- HANDSHAKE ---
//     // 1) send challenge
//     let mut nonce = [0u8; 32];
//     OsRng.fill_bytes(&mut nonce);
//     println!("[SERVER] sending Challenge nonce={}", hex::encode(nonce));
//     send(&mut write_half, &Msg::Challenge { nonce_b64: B64.encode(nonce) }).await?;

//     // 2) receive client's Identity and verify
//     let (peer_name, peer_id) = match recv(&mut reader).await? {
//         Msg::Identity { name, node_id, pubkey_b64, sig_b64 } => {
//             println!("[SERVER] verifying client Identity: name={name}, node_id={}...", &node_id[..8]);
//             verify_identity(&nonce, &node_id, &pubkey_b64, &sig_b64)
//                 .context("client identity verification failed")?;
//             println!("[SERVER] ✅ client authed: name={name} id={}", &node_id[..8]);
//             (name, node_id)
//         }
//         other => anyhow::bail!("expected Identity, got {other:?}"),
//     };

//     // 3) send our Identity (mutual auth) + Welcome
//     let sig_b64 = sign_nonce(&my_sk, &nonce);
//     send(&mut write_half, &Msg::Identity {
//         name: my_name.clone(),
//         node_id: my_id.clone(),
//         pubkey_b64: B64.encode(my_vk.as_bytes()),
//         sig_b64,
//     }).await?;
//     send(&mut write_half, &Msg::Welcome {
//         text: format!("Welcome {peer_name}, connected to {my_name} (id {}...)", &my_id[..8]),
//     }).await?;
//     println!("[SERVER] handshake complete with {peer_name} ({})", &peer_id[..8]);

//     // --- APP LOOP ---
//     loop {
//         match recv(&mut reader).await {
//             Ok(Msg::App { from, text }) => {
//                 println!("[SERVER] msg from {}: {}", &from[..8], text);
//                 send(&mut write_half, &Msg::Ack { text: format!("received: {text}") }).await?;
//             }
//             Ok(other) => {
//                 println!("[SERVER] unexpected message: {:?}", other);
//             }
//             Err(e) => {
//                 eprintln!("[SERVER] connection closed/err: {e}");
//                 break;
//             }
//         }
//     }
//     Ok(())
// }

// async fn run_client(addr: String, port: u16, name: String) -> Result<()> {
//     // Node2 keys
//     let (my_sk, my_vk, _seed) = gen_keypair("NODE2 (client)");
//     let my_id = node_id(&my_vk);

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
//         name: name.clone(),
//         node_id: my_id.clone(),
//         pubkey_b64: B64.encode(my_vk.as_bytes()),
//         sig_b64: sig_b64.clone(),
//     }).await?;
//     println!(
//         "[CLIENT] sent Identity: name={name}, my_id={}, pubkey={}, sig={}",
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
//     let mut stdin_lines = BufReader::new(tokio::io::stdin()).lines();
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

// // --- line-framed JSON helpers over TCP ---
// async fn send(w: &mut tokio::net::tcp::OwnedWriteHalf, msg: &Msg) -> Result<()> {
//     let s = serde_json::to_string(msg)?;
//     w.write_all(s.as_bytes()).await?;
//     w.write_all(b"\n").await?;
//     Ok(())
// }

// async fn recv(r: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> Result<Msg> {
//     let mut line = String::new();
//     let n = r.read_line(&mut line).await?;
//     if n == 0 {
//         anyhow::bail!("eof");
//     }
//     Ok(serde_json::from_str::<Msg>(line.trim())?)
// }


mod cli;
mod crypto;
mod io;
mod protocol;
mod net;

// NEW modules:
mod consensus;
mod chain;
mod rpc;

use anyhow::Result;
use clap::Parser;
use cli::{Args, Cmd};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    match args.cmd {
        Cmd::Listen { port, name } => net::run_server(port, name).await,
        Cmd::Connect { addr, port, name } => net::run_client(addr, port, name).await,
    }
}
