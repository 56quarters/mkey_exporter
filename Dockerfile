FROM rust:slim-bookworm AS BUILD
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=BUILD target/release/mkey_exporter /usr/local/bin/
CMD ["mkey_exporter", "--help"]
