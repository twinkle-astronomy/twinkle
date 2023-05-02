FROM rust

RUN apt-get update && apt install -y --no-install-recommends libcfitsio-dev

WORKDIR /app
COPY . /app

RUN cargo build --release

# ENTRYPOINT cargo run --release -- 
