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
    config_path.push(".openssl");

    macro_rules! load {
        ($file_name:expr) => {
            {
                println!("cargo:rerun-if-changed={}", config_path.join($file_name).display());
                let mut fd = File::open(config_path.join($file_name))
                                  .expect(concat!("opening ", $file_name));
                let mut bytes = vec![];
                fd.read_to_end(&mut bytes).expect(concat!("reading ", $file_name));
                bytes
            }
        };
    }

    let slave_key = load!("slave.rsa");
    let slave_cert_chain = load!("slave.fullchain");
    let master_key = load!("master.rsa");
    let master_cert_chain = load!("master.fullchain");
    let root_cert = load!("root_ca.cert");

    // `config.rs` is used by both master and slave.
    // (it contains secrets packed and shipped along with the executables)
    let source_path = PathBuf::from(&out_dir).join("config.rs");
    let contents = format!("
mod config {{
    use rustls::{{AllowAnyAuthenticatedClient, ClientConfig, ServerConfig}};
    use rustls::{{Certificate, PrivateKey, RootCertStore}};
    use rustls::internal::pemfile;

    use std::io::BufReader;
    use std::sync::Arc;

    const _CA_CERT: &'static [u8] = &{:?};
    const _MASTER_KEY: &'static [u8] = &{:?};
    const _MASTER_CERTS: &'static [u8] = &{:?};
    const _SLAVE_KEY: &'static [u8] = &{:?};
    const _SLAVE_CERTS: &'static [u8] = &{:?};

    fn load_cert(bytes: &[u8]) -> Vec<Certificate> {{
        let mut reader = BufReader::new(bytes);
        pemfile::certs(&mut reader).expect(\"loading certificate\")
    }}

    fn load_key(bytes: &[u8]) -> PrivateKey {{
        let mut reader = BufReader::new(bytes);
        let mut keys = pemfile::rsa_private_keys(&mut reader).expect(\"loading key\");
        keys.truncate(1);
        keys.pop().expect(\"key not found\")
    }}

    lazy_static! {{
        static ref _CONFIG: (Arc<ServerConfig>, Arc<ClientConfig>) = {{
            let slave_key = load_key(_SLAVE_KEY);
            let slave_certs = load_cert(_SLAVE_CERTS);
            let master_key = load_key(_MASTER_KEY);
            let master_certs = load_cert(_MASTER_CERTS);

            let mut root_ca_reader = BufReader::new(_CA_CERT);
            let mut root_store = RootCertStore::empty();
            root_store.add_pem_file(&mut root_ca_reader).unwrap();

            let client_auth = AllowAnyAuthenticatedClient::new(root_store.clone());
            let mut server_config = ServerConfig::new(client_auth);
            server_config.set_single_cert(slave_certs, slave_key);

            let mut client_config = ClientConfig::new();
            client_config.set_single_client_cert(master_certs, master_key);
            client_config.root_store = root_store;

            (Arc::new(server_config), Arc::new(client_config))
        }};

        pub static ref SERVER_CONFIG: &'static Arc<ServerConfig> = &_CONFIG.0;
        pub static ref CLIENT_CONFIG: &'static Arc<ClientConfig> = &_CONFIG.1;
    }}
}}
    ", root_cert, master_key, master_cert_chain, slave_key, slave_cert_chain);

    let mut fd = File::create(&source_path).unwrap();
    fd.write_all(contents.as_bytes()).unwrap();
}
