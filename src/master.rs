use ProcessType;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

pub struct Cluster {
    addrs: Vec<String>,     // FIXME: Change addrs to ToSocketAddr impls
}

impl Cluster {
    pub fn new() -> Cluster {
        Cluster {
            addrs: Vec::new(),
        }
    }

    fn connect_with_proc(&self, proc_type: ProcessType, addr: &str) -> Result<TcpStream, String> {
        let mut stream = TcpStream::connect(&addr)
                                   .map_err(|e| format!("Cannot connect to {} ({})", addr, e))?;
        proc_type.into_stream(&mut stream).map_err(|e| format!("Cannot ping {} ({})", addr, e))?;
        Ok(stream)
    }

    pub fn add_node(&mut self, addr: &str) -> Result<(), String> {
        let _ = self.ping_addr(addr)?;
        self.addrs.push(addr.to_owned());
        Ok(())
    }

    pub fn ping_addr(&self, addr: &str) -> Result<(), String> {
        let mut stream = self.connect_with_proc(ProcessType::Ping, addr)?;
        let mut response = [0; 1];
        let _ = stream.read_exact(&mut response);
        if response[0] > 0 {
            Ok(())
        } else {
            Err(format!("Failure receiving message from address: {}", addr))
        }
    }

    #[inline]
    pub fn ping_all(&self) -> Result<(), String> {
        for addr in &self.addrs {
            self.ping_addr(addr)?;
        }

        Ok(())
    }

    pub fn execute_at_attr(&self, addr: &str, command: &str) -> Result<StreamingOutput, String> {
        let mut stream = self.connect_with_proc(ProcessType::Execute, addr)?;
        stream.write_all(command.as_bytes()).map_err(|e| format!("Cannot write to stream ({})", e))?;
        Ok(StreamingOutput {
            buf: BufReader::new(stream),
        })
    }

    pub fn execute_all(&self) -> Result<(), String> {
        for addr in &self.addrs {
            self.execute_at_attr(addr)?;
        }

        Ok(())
    }
}

pub struct StreamingOutput {
    buf: BufReader<TcpStream>,
}

impl Iterator for StreamingOutput {
    type Item = Result<String, String>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut bytes = Vec::new();
        match self.buf.read_until(10, &mut bytes) {
            Ok(0) => None,
            Ok(_) => {
                let string = String::from_utf8_lossy(&bytes);
                Some(Ok(string.into_owned()))
            },
            Err(e) => Some(Err(format!("Error reading TCP stream ({})", e))),
        }
    }
}
