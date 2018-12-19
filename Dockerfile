# ------------------------------------
# builder image, contains all dev-deps
# ------------------------------------

FROM debian:unstable as builder

WORKDIR /root

# required packages
RUN apt-get update && \
    apt-get install --no-install-recommends -y \
        ca-certificates curl file \
        build-essential \
        clang \
        pkg-config \
        cmake \
        zlib1g \
        zlib1g-dev \
        ffmpeg \
        libsqlite3-0 \
        libsqlite3-dev \
        libavcodec-dev \
        libavformat-dev \
        libavfilter-dev \
        libssl-dev \
        libavdevice-dev \
        libavresample-dev \
        autoconf automake autotools-dev libtool xutils-dev && \
    rm -rf /var/lib/apt/lists/*

# install toolchain using rustup
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly-2018-12-08 -y
ENV PATH=/root/.cargo/bin:$PATH

RUN RUSTFLAGS="--cfg procmacro2_semver_exempt" cargo install cargo-tarpaulin



# ------------------------
# actually build the thing
# ------------------------

FROM builder

ADD . vorleser-server
RUN cd vorleser-server && cargo build --release



# ------------------------------------------
# final image with runtime deps and app only
# ------------------------------------------

FROM debian:unstable

RUN apt-get update && \
    apt-get install --no-install-recommends -y \
        ffmpeg \
        libsqlite3-0 \
        libssl1.1 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=1 /root/vorleser-server/target/release/vorleser_server_bin /usr/bin/vorleser-server

VOLUME /var/lib/vorleser

RUN printf '\n\
    database = "/var/lib/vorleser/vorleser.sqlite" \n\
    data_directory = "/var/lib/vorleser" \n\
    register_web = false \n\
    [scan] \n\
    enabled = true \n\
    interval = 600 \n\
    [logging] \n\
    level = "info" \n\
    [web] \n\
    address = "0.0.0.0" \n\
    port = 8000 \n\
    ' >> /etc/vorleser.toml

EXPOSE 8000

ENTRYPOINT ["vorleser-server", "-c", "/etc/vorleser.toml"]
CMD ["serve"]
