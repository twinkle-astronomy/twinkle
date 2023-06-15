FROM rust as dev

RUN apt-get update && apt-get install -y --no-install-recommends libcfitsio-dev libopencv-dev clang libclang-dev && rm -rf /var/lib/apt/lists/*

RUN mkdir /app
ENV HOME="/app"
WORKDIR /app


#This user schenanigans allows for local development
ARG USER=app
ARG USER_ID=1000
ARG GROUP_ID=1000

RUN groupadd -g ${GROUP_ID} ${USER} && \
    useradd -l -u ${USER_ID} -g ${USER} ${USER}

RUN chown ${USER}:${USER} /app
USER ${USER}

from dev as builder
COPY . /app
RUN cargo install --path ./phd2_exporter
RUN cargo install --path ./indi_exporter

FROM debian:bullseye-slim as release
# RUN apt-get update && apt-get install -y extra-runtime-dependencies && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/cargo/bin/phd2_exporter /usr/local/bin/
COPY --from=builder /usr/local/cargo/bin/indi_exporter /usr/local/bin/
