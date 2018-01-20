use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:rerun-if-changed=build.rs");            // rebuild if this file has changed
    let root = env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut config_path = PathBuf::from(&root);
    config_path.pop();
    config_path.push("build");

    // rebuild if key/cert changes
    println!("cargo:rerun-if-changed={}", config_path.join("server.crt").display());
    println!("cargo:rerun-if-changed={}", config_path.join("server.pem").display());

    let mut fd = File::open(config_path.join("server.pem")).expect("opening private key");
    let mut private_key = vec![];
    fd.read_to_end(&mut private_key).expect("reading private key");

    let mut fd = File::open(config_path.join("server.crt")).expect("opening certificate");
    let mut cert = vec![];
    fd.read_to_end(&mut cert).expect("reading certificate");

    let source_path = PathBuf::from(&out_dir).join("config.rs");
    let contents = format!("
mod config {{
    use rustls::{{Certificate, PrivateKey}};
    use rustls::internal::pemfile;

    use std::io::BufReader;

    const _CERT: &'static [u8] = &{:?};
    const _KEY: &'static [u8] = &{:?};

    lazy_static! {{
        pub static ref CERTIFICATE: Vec<Certificate> = {{
            let mut reader = BufReader::new(_CERT);
            pemfile::certs(&mut reader).expect(\"loading certificate\")
        }};

        pub static ref PRIVATE_KEY: PrivateKey = {{
            let mut reader = BufReader::new(_KEY);
            let mut keys = pemfile::rsa_private_keys(&mut reader).expect(\"loading keys\");
            keys.truncate(1);
            keys.pop().expect(\"no keys found\")
        }};
    }}
}}
    ", cert, private_key);

    let mut fd = File::create(&source_path).unwrap();
    fd.write_all(contents.as_bytes()).unwrap();
}
