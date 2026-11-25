use std::{fs::File, io::Read, path::PathBuf};

use anyhow::{Context, Result, ensure};
use clap::Parser;
use packfs::cpk::CpkArchive;

#[derive(Parser)]
#[command(about = "Read-only inspection of PAL4 database and UI archives")]
struct Cli {
    /// Path to a shipped CPK archive.
    #[arg(long)]
    archive: PathBuf,

    /// Archive-relative file to inspect; omit to list files.
    #[arg(long)]
    entry: Option<String>,

    /// Print 512 bytes at this byte offset instead of decoding a database.
    #[arg(long, requires = "entry")]
    hex_offset: Option<usize>,

    /// Decode only this named database table.
    #[arg(long, requires = "entry", conflicts_with_all = ["hex_offset", "enumeration"])]
    table: Option<String>,

    /// Decode only this named database enum.
    #[arg(long, requires = "entry", conflicts_with = "hex_offset")]
    enumeration: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut archive = CpkArchive::load(Box::new(
        File::open(&cli.archive).with_context(|| format!("open {}", cli.archive.display()))?,
    ))?;
    if let Some(entry) = cli.entry {
        let mut bytes = Vec::new();
        archive
            .open_str(&entry)
            .with_context(|| format!("open archive entry {entry}"))?
            .read_to_end(&mut bytes)?;
        let is_database = entry.to_ascii_lowercase().ends_with(".db");
        ensure!(
            is_database || (cli.table.is_none() && cli.enumeration.is_none()),
            "--table and --enumeration require a .db entry"
        );
        if cli.hex_offset.is_none() && is_database {
            let database = fileformats::pal4::database::Database::read(&bytes)?;
            let output = if let Some(name) = cli.table {
                serde_json::to_string_pretty(
                    database
                        .tables
                        .iter()
                        .find(|t| t.name == name)
                        .with_context(|| format!("database table not found: {name}"))?,
                )?
            } else if let Some(name) = cli.enumeration {
                serde_json::to_string_pretty(
                    database
                        .enums
                        .iter()
                        .find(|e| e.name == name)
                        .with_context(|| format!("database enum not found: {name}"))?,
                )?
            } else {
                serde_json::to_string_pretty(&database)?
            };
            println!("{output}");
            return Ok(());
        }
        println!(
            "{}: {} bytes, md5 {:x}",
            entry,
            bytes.len(),
            md5::compute(&bytes)
        );
        let offset = cli.hex_offset.unwrap_or(0);
        let bytes = bytes
            .get(offset..)
            .context("hex offset past end of entry")?;
        for (i, chunk) in bytes.chunks(16).take(32).enumerate() {
            println!(
                "{:08x}  {}  {}",
                offset + i * 16,
                chunk
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" "),
                chunk
                    .iter()
                    .map(|&b| if b.is_ascii_graphic() { b as char } else { '.' })
                    .collect::<String>()
            );
        }
    } else {
        let paths = archive.full_paths()?;
        let mut files: Vec<_> = paths
            .iter()
            .zip(&archive.entries)
            .filter(|(_, entry)| entry.is_file() && !entry.is_dir())
            .collect();
        files.sort_by_key(|(path, _)| path.to_ascii_lowercase());
        for (path, _) in files {
            println!("{path}");
        }
    }
    Ok(())
}
