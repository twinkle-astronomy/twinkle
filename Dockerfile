FROM rust as dev

RUN apt-get update && apt install -y --no-install-recommends libcfitsio-dev

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

