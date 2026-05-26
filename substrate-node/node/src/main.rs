#![warn(missing_docs)]

//! Verifiable ML Prediction Market node.

mod chain_spec;
mod cli;
mod command;
mod rpc;
mod service;

fn main() -> sc_cli::Result<()> {
    command::run()
}
