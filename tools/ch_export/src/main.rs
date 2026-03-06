use anyhow::{Context, Result};
use clap::Parser;
use futures_util::StreamExt;
use reqwest::Client;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};

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

    let file = File::create(&args.output)
        .await
        .with_context(|| format!("Failed to create output file: {:?}", args.output))?;
    
    // Use an 8MB buffer to minimize syscall overhead for many small network chunks
    let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, file);

    let mut stream = response.bytes_stream();
    let mut bytes_written = 0_usize;
    let mut last_print = Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading response stream")?;
        writer.write_all(&chunk)
            .await
            .context("Failed to write to file")?;
        bytes_written += chunk.len();
        
        let now = Instant::now();
        if now.duration_since(last_print).as_millis() > 500 {
            print!("\rDownloaded: {:.2} MB", bytes_written as f64 / 1_048_576.0);
            std::io::stdout().flush()?;
            last_print = now;
        }
    }
    
    // Print the final progress
    print!("\rDownloaded: {:.2} MB", bytes_written as f64 / 1_048_576.0);
    std::io::stdout().flush()?;
    
    // Ensure the remaining buffer is actually flushed to the OS
    writer.flush().await.context("Failed to flush buffer to file")?;

    println!(
        "\nExport completed successfully! Total bytes: {}",
        bytes_written
    );

    Ok(())
}
