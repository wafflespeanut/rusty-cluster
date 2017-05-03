use {BUFFER_SIZE, Data, ProcessType};
use utils::AsBytes;

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;

pub struct Cluster {
    addrs: HashSet<String>,     // FIXME: Change addrs to ToSocketAddr impls
}

impl Cluster {
    pub fn new() -> Cluster {
        Cluster {
            addrs: HashSet::new(),
        }
    }

    fn connect_with_proc(&self, proc_type: ProcessType, addr: &str) -> Result<TcpStream, String> {
        let mut stream = TcpStream::connect(&addr)
                                   .map_err(|e| format!("Cannot connect to {} ({})", addr, e))?;
        proc_type.into_stream(&mut stream).map_err(|e| format!("Cannot ping {} ({})", addr, e))?;
        Ok(stream)
    }

    pub fn add_node(&mut self, addr: &str) -> Result<(), String> {
        if self.addrs.contains(addr) {
            return Ok(())
        }

        let _ = self.ping_addr(addr)?;
        self.addrs.insert(addr.to_owned());
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

    pub fn execute_at_node<C>(&self, addr: &str, command: &C) -> Result<StreamingOutput, String>
        where C: AsBytes
    {
        let stream = self.connect_with_proc(ProcessType::Execute, addr)?;
        let data = Data(command.bytes());
        data.serialize_into(&stream)?;
        Ok(StreamingOutput {
            buf: BufReader::with_capacity(BUFFER_SIZE, stream),
        })
    }

    #[inline]
    pub fn execute_all<C: AsBytes>(&self, command: &C) -> Result<(), String> {
        for addr in &self.addrs {
            self.execute_at_node(addr, command)?;
        }

        Ok(())
    }
}

pub struct StreamingOutput {
    buf: BufReader<TcpStream>,
}

impl Iterator for StreamingOutput {
    // It's bytes because we basically want to print/write the result of
    // execution, and Write implementors only take byte slices in the end anyway.
    // So, no reason to convert to strings along the way.
    type Item = Result<Vec<u8>, String>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut bytes = Vec::new();
        match self.buf.read_until(10, &mut bytes) {
            Ok(0) => None,
            Ok(_) => Some(Ok(bytes)),
            Err(e) => Some(Err(format!("Error reading TCP stream ({})", e))),
        }
    }
}
