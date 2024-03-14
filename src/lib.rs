pub mod alloc;
#[cfg(target_family = "unix")]
pub mod daemon;
pub mod serve;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::{net::SocketAddr, path::PathBuf};

#[derive(Parser)]
#[clap(author, version, about, arg_required_else_help = true)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Opt {
    #[clap(subcommand)]
    pub commands: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run server
    Run(BootArgs),
    /// Start server daemon
    #[cfg(target_family = "unix")]
    Start(BootArgs),
    /// Restart server daemon
    #[cfg(target_family = "unix")]
    Restart(BootArgs),
    /// Stop server daemon
    #[cfg(target_family = "unix")]
    Stop,
    /// Show the server daemon log
    #[cfg(target_family = "unix")]
    Log,
    /// Show the server daemon process
    #[cfg(target_family = "unix")]
    PS,
}

#[derive(Args, Clone, Debug)]
pub struct BootArgs {
    /// Debug mode
    #[clap(short, long)]
    pub debug: bool,

    /// Bind address
    #[clap(short, long, default_value = "0.0.0.0:8000")]
    pub bind: SocketAddr,

    /// TLS certificate file
    #[clap(long)]
    pub tls_cert: Option<PathBuf>,

    /// TLS private key file
    #[clap(long)]
    pub tls_key: Option<PathBuf>,

    /// API key
    #[clap(short = 'A', long)]
    pub api_key: Option<String>,

    /// Deepl `dl_session`
    #[clap(long)]
    pub dl_session: Option<String>,

    /// Deepl client proxy
    #[clap(short = 'x',long, env = "PROXIES", value_parser = parse_proxies_url, verbatim_doc_comment)]
    pub proxies: Option<std::vec::Vec<String>>,
}

fn parse_proxies_url(s: &str) -> Result<std::vec::Vec<String>> {
    Ok(s.split(',').map(|s| s.to_string()).collect())
}
