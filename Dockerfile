FROM liuchong/rustup:nightly

RUN apt update
RUN apt-get install build-essential pkg-config ffmpeg libavcodec-dev libavformat-dev libavfilter-dev libpq-dev libavdevice-dev libavresample-dev clang -y
RUN cargo install diesel_cli --no-default-features --features "postgres" --verbose
ENV PATH /root/.cargo/bin:$PATH
