FROM ubuntu:22.04

RUN apt-get update
RUN DEBIAN_FRONTEND="noninteractive" apt-get install -y \
    software-properties-common

RUN apt-add-repository ppa:mutlaqja/ppa
RUN DEBIAN_FRONTEND="noninteractive" apt-get install -y \
    indi-full \
    gsc
