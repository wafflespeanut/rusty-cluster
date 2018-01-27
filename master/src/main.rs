extern crate rcluster;
extern crate structopt;
#[macro_use] extern crate structopt_derive;

use rcluster::{Master, utils};
use rcluster::errors::ClusterResult;
use structopt::StructOpt;

use std::error::Error;
use std::net::SocketAddr;

#[derive(StructOpt, Debug)]
enum FileSync {
    #[structopt(name = "send")]
    /// Send file to slave machine
    SendOne {
        #[structopt(long = "from")]
        source: String,
        #[structopt(long = "to")]
        dest: String,
    },
    #[structopt(name = "receive")]
    /// Receive file from slave machine
    ReceiveOne {
        #[structopt(long = "from")]
        source: String,
        #[structopt(long = "to")]
        dest: String,
    }
}

// Structure solely for obtaining the command-line arguments.
#[derive(StructOpt)]
struct Options {
    #[structopt(help = "Address of slave machine")]
    address: SocketAddr,
    #[structopt(short = "p", long = "ping", help = "Ping the slave service")]
    ping: bool,
    #[structopt(subcommand)]
    file: Option<FileSync>,
}

fn handle_request() -> ClusterResult<()> {
    let options = Options::from_args();
    let mut master = Master::new();
    let id = master.add_slave(options.address)?;
    if options.ping {
        master.ping(id)?;
        println!("Successfully pinged slave!");
    }

    match options.file {
        Some(FileSync::SendOne { source, dest }) => {
            master.send_file(id, source, dest)?;
            println!("Successfully sent file!");
        },
        _ => (),
    }

    Ok(())
}

fn main() {
    utils::prepare_logger();
    if let Err(e) = handle_request() {
        println!("ERROR: {}", e.description());
    }
}
