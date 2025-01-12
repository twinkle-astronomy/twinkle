FROM rust:1.80-bullseye as dev

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

from dev as phd2_exporter_builder
COPY . /app
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo install --target x86_64-unknown-linux-musl --path ./phd2_exporter

from dev as indi_exporter_builder
COPY . /app
RUN cargo install --path ./indi_exporter

FROM debian:bullseye-slim as indi_exporter-release
COPY --from=indi_exporter_builder /usr/local/cargo/bin/indi_exporter /usr/local/bin/

FROM scratch as phd2_exporter-release
COPY --from=phd2_exporter_builder /usr/local/cargo/bin/phd2_exporter /usr/local/bin/phd2_exporter
