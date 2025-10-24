use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name="tcp-handshake-demo", about="TCP handshake + signed identity + message transfer")]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Run Node1: listen on a port
    Listen {
        #[arg(long, default_value_t = 7000)]
        port: u16,
        #[arg(long, default_value = "auto")]
        name: String,
    },
    /// Run Node2: connect to Node1 at addr:port
    Connect {
        #[arg(long)]
        addr: String,
        #[arg(long)]
        port: u16,
        #[arg(long, default_value = "auto")]
        name: String,
    },
}
