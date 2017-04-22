use ProcessType;

use std::io::{BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;

pub struct Slave {
    sender: Sender<()>,
    receiver: Receiver<()>,
    exit_flag: bool,
}

impl Slave {
    pub fn new() -> Slave {
        let (sender, receiver) = mpsc::channel();
        Slave {
            sender: sender,
            receiver: receiver,
            exit_flag: false,
        }
    }

    pub fn handle_stream(&mut self, stream: TcpStream) {
        let mut reader = BufReader::new(stream);
        let proc_type = match ProcessType::from_stream(&mut reader) {
            Ok(p) => p,
            Err(_) => return,
        };

        let sender = self.sender.clone();
        let _ = thread::spawn(move || {
            let mut writer = BufWriter::new(reader.into_inner());
            let _ = writer.write(&[1]);     // ack

            match proc_type {
                ProcessType::Shutdown => {
                    let _ = sender.send(());
                },
                _ => (),
            }
        });

        if proc_type == ProcessType::Shutdown {
            self.exit_flag = true;
        }
    }

    pub fn start_listening(&mut self, addr: &str) {        // Slave is a 'quiet' (infinite) listener by design
        let listener = match TcpListener::bind(addr) {
            Ok(l) => l,
            Err(_) => return,
        };

        for stream in listener.incoming().filter_map(|s| s.ok()) {
            self.handle_stream(stream);

            if self.exit_flag {
                let _ = self.receiver.recv();
                break
            }
        }
    }
}
