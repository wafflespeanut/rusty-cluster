extern crate rcluster;

use rcluster::Master;

fn main() {
    Master::test_connection();
}
