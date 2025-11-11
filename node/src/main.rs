//! Substrate Node Template CLI library.
#![warn(missing_docs)]

mod benchmarking;
mod chain_spec;
mod cli;
mod command;
mod rpc;
mod rpc_trust_score;
mod service;

fn main() -> sc_cli::Result<()> {
    command::run()
}
