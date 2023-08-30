use clap::Parser;
use futures_util::FutureExt;
use jsonrpc_socket_client::client::builder::SocketClientBuilder;
use jsonrpsee_core::client::ClientT;
use jsonrpsee_core::JsonValue;

use jsonrpsee_core::{params::ArrayParams, rpc_params};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    ip: String,

    #[arg(short, long)]
    port: usize,

    #[arg(short, long)]
    method: String,
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let url = format!("{}:{}", args.ip, args.port);
    let builder = SocketClientBuilder::new();
    let client = builder.build(url).await?;

    // Throwaway call to force the request id to 1 for real calls
    client
        .request::<String, ArrayParams>("", rpc_params![])
        .now_or_never();

    let response = client
        .request::<JsonValue, ArrayParams>(&args.method, rpc_params![])
        .await?;
    println!("{:?}", response);

    Ok(())
}
