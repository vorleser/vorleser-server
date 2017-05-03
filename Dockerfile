FROM liuchong/rustup:nightly

RUN apt update
RUN rustup install nightly
RUN apt-get install build-essential pkg-config ffmpeg libavcodec-dev libavformat-dev libpq-dev -y
RUN cargo install diesel_cli --no-default-features --features "postgres" --verbose
ENV PATH /root/.cargo/bin:$PATH
