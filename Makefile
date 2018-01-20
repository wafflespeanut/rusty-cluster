NORMAL_PATH = target/release
MUSL_PATH = target/x86_64-unknown-linux-musl/release

create-certs:
	openssl req -nodes -newkey rsa:4096 -x509 \
		-keyout build/server.key -new -out build/server.crt \
		-config build/openssl-cert.cnf -sha256 -days 7500
	openssl rsa -in build/server.key -text > build/server.pem
	rm build/server.key

build: create-certs
	cd master && cargo build
	cd slave && cargo build

build-release: create-certs
	cd master && cargo build --release
	cd slave && cargo build --release
