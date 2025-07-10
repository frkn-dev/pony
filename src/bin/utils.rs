use clap::{Parser, Subcommand};
use pony::{Message, Publisher};
use rkyv::to_bytes;
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "utils")]
#[command(about = "Helper tool for generating and sending ZMQ messages", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate rkyv .bin message from JSON
    Gen {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Send a .bin rkyv message to ZMQ
    Send {
        #[arg(short, long)]
        topic: String,
        #[arg(short, long)]
        input: PathBuf,

        /// Publisher address, e.g. tcp://127.0.0.1:3000
        #[arg(short, long, default_value = "tcp://127.0.0.1:3000")]
        addr: String,
    },

    /// Generate and send in one go
    All {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        bin: PathBuf,
        #[arg(short, long)]
        topic: String,

        /// Publisher address, e.g. tcp://127.0.0.1:3000
        #[arg(short, long, default_value = "tcp://127.0.0.1:3000")]
        addr: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Gen { input, output } => {
            let json = fs::read_to_string(input)?;
            let msg: Message = serde_json::from_str(&json)?;
            let bytes = to_bytes::<_, 1024>(&msg)?;
            fs::write(&output, &bytes)?;
            println!("✅ rkyv message written to {:?}", output);
        }

        Commands::Send { topic, input, addr } => {
            let bytes = fs::read(input)?;
            let publisher = Publisher::new(&addr).await;
            publisher.send_binary(&topic, &bytes).await?;
            println!("✅ rkyv message sent to topic: {}", topic);
        }

        Commands::All {
            input,
            bin,
            topic,
            addr,
        } => {
            let json = fs::read_to_string(&input)?;
            let msg: Message = serde_json::from_str(&json)?;
            let bytes = to_bytes::<_, 1024>(&msg)?;
            fs::write(&bin, &bytes)?;
            println!("✅ [Step 1] Message written to {:?}", bin);

            let publisher = Publisher::new(&addr).await;
            publisher.send_binary(&topic, &bytes).await?;
            println!("✅ [Step 2] Message sent to topic: {}", topic);
        }
    }

    Ok(())
}
