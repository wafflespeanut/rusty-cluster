extern crate rand;
extern crate rcluster;

use rcluster::slave::Slave;
use rcluster::master::Cluster;

use std::net::TcpListener;
use std::thread;
use std::time::Duration;

const LOCAL_ADDR: &'static str = "127.0.0.1";
const NUM_SLAVES: usize = 4;

#[test]
fn test_cluster() {
    // Test at localhost with multiple slaves in multiple ports
    // (should mimic slaves at different ports in different machines)
    let mut master = Cluster::new();
    let mut addrs = vec![];
    while addrs.len() < NUM_SLAVES {    // since we can't be sure which ports are busy
        let rand_port = rand::random::<u16>();
        if TcpListener::bind((LOCAL_ADDR, rand_port)).is_ok() {
            addrs.push(format!("{}:{}", LOCAL_ADDR, rand_port));
        }
    }

    for a in &addrs {
        let a = a.clone();
        let _ = thread::spawn(move || {
            Slave::new().start_listening(&a);
        });
    }

    let time = Duration::from_millis(100);      // need some time to bind
    thread::sleep(time);

    for a in &addrs {
        master.add_node(&a).unwrap();
    }
}
