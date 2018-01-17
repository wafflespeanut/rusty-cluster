extern crate rcluster;

use rcluster::{Slave, utils};

fn main() {
    utils::prepare_logger();
    Slave::default().start_listening();
}
