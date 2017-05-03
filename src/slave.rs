use {BUFFER_SIZE, Data, ProcessType, utils};

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
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

        macro_rules! get_data {
            ($stream:expr) => {
                match Data::deserialize_from(&$stream) {
                    Ok(s) => s.0,
                    Err(e) => {
                        let _ = $stream.write(e.as_bytes());
                        return
                    },
                }
            };
        }

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
                    let cmd_stream: Vec<u8> = get_data!(stream);
                    let mut args = utils::split_args(cmd_stream);
                    let cmd = args.remove(0);
                    if cmd.is_empty() {
                        let _ = stream.write("No input given".as_bytes());
                        return
                    }

                    let mut writer = BufWriter::new(stream);
                    let child = Command::new(cmd).args(&args)
                                                 .stdout(Stdio::piped())
                                                 .stderr(Stdio::piped())
                                                 .spawn();
                    let mut process = match child {
                        Ok(c) => c,
                        Err(e) => {     // try to send the error back to the server
                            let _ = writer.write(format!("Error running command: {}", e).as_bytes());
                            return
                        }
                    };

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
                ProcessType::Write => {
                    let path: String = get_data!(stream);
                    let mut fd = match File::create(&path) {
                        Ok(f) => BufWriter::new(f),
                        Err(_) => return,
                    };

                    let mut reader = BufReader::with_capacity(BUFFER_SIZE, stream);
                    loop {
                        let mut bytes = Vec::new();
                        let mut chunk = (&mut reader).take(BUFFER_SIZE as u64);
                        match chunk.read_to_end(&mut bytes) {
                            Ok(0) | Err(_) => break,
                            _ => {
                                let _ = fd.write(&bytes);
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
