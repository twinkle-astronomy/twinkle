FROM rust:1.84-bullseye AS dev

RUN apt-get update && apt-get install -y --no-install-recommends \
    libcfitsio-dev \
    libopencv-dev \
    clang \
    libclang-dev \
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

RUN cargo install trunk

FROM dev AS twinkle-build
COPY . /app
USER root

# Build server
RUN cargo build --release --bin server
# Build frontend
RUN ls -lah /app/egui-frontend
RUN cd egui-frontend && trunk build --release

FROM debian:bullseye-slim AS twinkle
RUN apt-get update && apt-get install -y --no-install-recommends \
    libcfitsio-dev \
    && rm -rf /var/lib/apt/lists/*
RUN mkdir /app
COPY --from=twinkle-build /app/target/release/server /app/server
COPY --from=twinkle-build /app/egui-frontend/dist /app/assets
USER ${USER}
WORKDIR /app
