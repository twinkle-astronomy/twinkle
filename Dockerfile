FROM rust:1.83-bookworm AS dev

RUN apt-get update && apt-get install -y --no-install-recommends \
    libcfitsio-dev \
    libopencv-dev \
    clang \
    libclang-dev \
    libgtk-4-dev build-essential \
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
RUN cargo install --locked trunk

FROM dev AS phd2_exporter_builder
COPY . /app
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo install --target x86_64-unknown-linux-musl --path ./phd2_exporter

FROM dev AS indi_exporter_builder
COPY . /app
RUN cargo install --path ./indi_exporter

FROM debian:bullseye-slim AS indi_exporter-release
COPY --from=indi_exporter_builder /usr/local/cargo/bin/indi_exporter /usr/local/bin/

FROM scratch AS phd2_exporter-release
COPY --from=phd2_exporter_builder /usr/local/cargo/bin/phd2_exporter /usr/local/bin/phd2_exporter
