use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
};
use crate::protocol::Msg;

pub async fn send(w: &mut OwnedWriteHalf, msg: &Msg) -> Result<()> {
    let s = serde_json::to_string(msg)?;
    w.write_all(s.as_bytes()).await?;
    w.write_all(b"\n").await?;
    Ok(())
}

pub async fn recv(r: &mut BufReader<OwnedReadHalf>) -> Result<Msg> {
    let mut line = String::new();
    let n = r.read_line(&mut line).await?;
    if n == 0 {
        anyhow::bail!("eof");
    }
    Ok(serde_json::from_str::<Msg>(line.trim())?)
}