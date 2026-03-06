use anyhow::{Context, Result};
use clap::Parser;
use futures_util::StreamExt;
use reqwest::Client;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// ClickHouse HTTP endpoint
    #[arg(short, long, default_value = "http://localhost:8123")]
    url: String,

    /// Database name
    #[arg(short, long, default_value = "db_trading")]
    db: String,

    /// Table name to export
    #[arg(short, long)]
    table: String,

    /// Output CSV file path
    #[arg(short, long, default_value = "output.csv")]
    output: PathBuf,

    /// ClickHouse username
    #[arg(long, default_value = "default")]
    user: String,

    /// ClickHouse password
    #[arg(long, default_value = "")]
    password: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!(
        "Exporting table '{}' from database '{}' to {:?}",
        args.table, args.db, args.output
    );
    println!("Connecting to ClickHouse at {}...", args.url);

    let query = format!(
        "SELECT * FROM {}.{} FORMAT CSVWithNames",
        args.db, args.table
    );

    let client = Client::new();
    let mut req = client.post(&args.url).body(query);

    if !args.password.is_empty() {
        req = req.basic_auth(&args.user, Some(&args.password));
    } else {
        req = req.header("X-ClickHouse-User", &args.user);
    }

    let response = req
        .send()
        .await
        .context("Failed to send request to ClickHouse")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        anyhow::bail!("ClickHouse error ({}): {}", status, error_text);
    }

    let mut file = File::create(&args.output)
        .with_context(|| format!("Failed to create output file: {:?}", args.output))?;

    let mut stream = response.bytes_stream();
    let mut bytes_written = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading response stream")?;
        file.write_all(&chunk).context("Failed to write to file")?;
        bytes_written += chunk.len();
        print!("\rDownloaded: {} KB", bytes_written / 1024);
        std::io::stdout().flush()?;
    }

    println!(
        "\nExport completed successfully! Total bytes: {}",
        bytes_written
    );

    Ok(())
}
