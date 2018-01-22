extern crate rcluster;

use rcluster::Slave;
use rcluster::errors::ClusterResult;
use rcluster::utils::{self, DEFAULT_ADDRESS};

use std::env;

fn start_listening() -> ClusterResult<()> {
    let addr = env::var("ADDRESS").unwrap_or(DEFAULT_ADDRESS.to_string()).parse()?;
    Slave::new(addr).start_listening()?;
    Ok(())
}

fn main() {
    utils::prepare_logger();
    if let Err(e) = start_listening() {
        println!("ERROR: {:?}", e);
    }
}
