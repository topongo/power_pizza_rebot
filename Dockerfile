FROM rust AS builder

WORKDIR /src
COPY . .

ARG PROFILE=release
RUN cargo build --profile ${PROFILE} --bin ppp_bot
RUN cargo build --profile ${PROFILE} --bin ppp_import

FROM debian:bookworm-slim as runtime

RUN apt-get -y update && apt-get -y install tini openssl ffmpeg

WORKDIR /app

RUN useradd -m -s /bin/bash -d /home/ppp ppp
RUN chown -R ppp:ppp /home/ppp /app
USER ppp

COPY --from=builder /src/target/release/ppp_bot /usr/local/bin/ppp_bot
COPY --from=builder /src/target/release/ppp_import /usr/local/bin/ppp_import

ENTRYPOINT ["/usr/bin/tini", "--"]
