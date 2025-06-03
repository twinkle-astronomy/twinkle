FROM rust:1.86-bookworm AS dev

RUN apt-get update && apt-get install -y --no-install-recommends \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*
RUN rustup component add rustfmt
RUN rustup target add wasm32-unknown-unknown

RUN mkdir /app
ENV HOME="/app"
WORKDIR /app


#This user schenanigans allows for local development
ARG USER=app
ARG USER_ID=1000
ARG GROUP_ID=1000

RUN groupadd -g ${GROUP_ID} ${USER} && \
    useradd -l -u ${USER_ID} -g ${USER} -s /bin/bash ${USER}

RUN chown ${USER}:${USER} /app
USER ${USER}

ENV DATABASE_URL sqlite:///storage/db.sqlite
RUN cargo install trunk diesel_cli

FROM dev AS twinkle-build
COPY . /app
USER root

# Build server
RUN cd twinkle_server && cargo build --release --bin server
# Build frontend
RUN cd egui-frontend && trunk build --release

FROM debian:bookworm-slim AS twinkle
RUN apt-get update && apt-get install -y --no-install-recommends \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*
RUN mkdir /app
COPY --from=twinkle-build /app/twinkle_server/target/release/server /app/server
COPY --from=twinkle-build /app/egui-frontend/dist /app/assets
USER ${USER}
WORKDIR /app
