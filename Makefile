CONFIG=build/openssl-cert.cnf
OUT_DIR=build/.openssl
ROOT_CA=$(OUT_DIR)/root_ca
SLAVE=$(OUT_DIR)/slave
MASTER=$(OUT_DIR)/master

create-certs:
	if [ -d $(OUT_DIR) ]; then \
		rm -rf $(OUT_DIR) ; \
	fi

	mkdir $(OUT_DIR)
	openssl req -nodes -newkey rsa:4096 -x509 \
		-keyout $(ROOT_CA).key -out $(ROOT_CA).cert \
		-sha256 -days 7500 -batch -subj "/CN=TLS snoop RSA CA"

	openssl req -nodes -newkey rsa:4096 -keyout $(SLAVE).key \
		-out $(SLAVE).req -sha256 -batch -subj "/CN=tls.snoop"

	openssl rsa -in $(SLAVE).key -out $(SLAVE).rsa

	openssl req -nodes -newkey rsa:4096 -keyout $(MASTER).key -out $(MASTER).req \
		-sha256 -batch -subj "/CN=snooper client"

	openssl rsa -in $(MASTER).key -out $(MASTER).rsa

	openssl x509 -req -in $(SLAVE).req -out $(SLAVE).cert -CA $(ROOT_CA).cert \
		-CAkey $(ROOT_CA).key -sha256 -days 3650 -set_serial 456 \
		-extensions v3_end -extfile $(CONFIG)

	openssl x509 -req -in $(MASTER).req -out $(MASTER).cert -CA $(ROOT_CA).cert \
		-CAkey $(ROOT_CA).key -sha256 -days 3650 -set_serial 789 \
		-extensions v3_client -extfile $(CONFIG)

	cat $(SLAVE).cert $(ROOT_CA).cert > $(SLAVE).fullchain
	cat $(MASTER).cert $(ROOT_CA).cert > $(MASTER).fullchain

build: create-certs
	cd master && cargo build
	cd slave && cargo build

build-release: create-certs
	cd master && cargo build --release
	cd slave && cargo build --release
