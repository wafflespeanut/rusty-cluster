extern crate rcluster;
extern crate structopt;
#[macro_use] extern crate structopt_derive;

use rcluster::{Master, utils};
use rcluster::errors::ClusterResult;
use structopt::StructOpt;

use std::error::Error;
use std::net::SocketAddr;

// Structure solely for obtaining the command-line arguments.
#[derive(StructOpt, Debug)]
pub struct Options {
    #[structopt(help = "Address of slave machine")]
    address: SocketAddr,
    #[structopt(short = "p", long = "ping", help = "Ping the slave service")]
    ping: bool,
}

fn handle_request() -> ClusterResult<()> {
    let options = Options::from_args();
    let mut master = Master::new();
    let id = master.add_slave(options.address)?;
    if options.ping {
        master.ping(id)?;
        println!("Successfully pinged slave!");
    }

    Ok(())
}

fn main() {
    utils::prepare_logger();
    if let Err(e) = handle_request() {
        println!("ERROR: {}", e.description());
    }
}
