use {ProcessType, utils};

use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::mem;
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
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

    pub fn handle_stream(&mut self, mut stream: TcpStream) {
        let proc_type = match ProcessType::from_stream(&mut stream) {
            Ok(p) => p,
            Err(_) => return,
        };

        let sender = self.sender.clone();
        let _ = thread::spawn(move || {
            match proc_type {
                ProcessType::Ping => {
                    let mut writer = BufWriter::new(stream);
                    let _ = writer.write(&[1]);
                },
                ProcessType::Shutdown => {
                    let _ = sender.send(());
                },
                ProcessType::Execute => {
                    let mut cmd_stream = Vec::new();
                    let _ = stream.read_to_end(&mut cmd_stream);        // careful - possible DOS
                    let mut args = utils::split_args(cmd_stream);
                    let cmd = mem::replace(&mut args[0], String::new());
                    if cmd.is_empty() {
                        let _ = stream.write("No input given".as_bytes());
                        return
                    }

                    let mut writer = BufWriter::new(stream);
                    let child = Command::new(cmd).args(&args[1..])
                                                 .stdout(Stdio::piped())
                                                 .stderr(Stdio::piped())
                                                 .spawn();
                    if let Err(e) = child {
                        let _ = writer.write(format!("Error running command: {}", e).as_bytes());
                        return
                    }

                    let mut process = child.unwrap();
                    let mut bytes = Vec::new();
                    let mut reader = BufReader::new(process.stdout.as_mut().unwrap());
                    loop {
                        match reader.read_until(10, &mut bytes) {
                            Ok(0) | Err(_) => break,
                            _ => {
                                let _ = writer.write(&bytes);
                                bytes.clear();
                            },
                        }
                    }
                },
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
