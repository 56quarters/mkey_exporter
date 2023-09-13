# mkey_exporter

![build status](https://github.com/56quarters/mkey_exporter/actions/workflows/rust.yml/badge.svg)
[![docs.rs](https://docs.rs/mkey_exporter/badge.svg)](https://docs.rs/mkey_exporter/)
[![crates.io](https://img.shields.io/crates/v/mkey_exporter.svg)](https://crates.io/crates/mkey_exporter/)

Export counts and sizes of Memcached keys matching regular expressions as Prometheus metrics.

For example, with the Memcached keys:

```
thing-1:user-1:something
thing-1:user-1:something-different
thing-1:user-2:something
thing-1:user-3:something
thing-2:user-1:something
thing-2:user-1:something-else
```

And the configuration:

```yaml
name: demo
rules:
- pattern: '^(\w+):'
  label_name: 'thing'
  label_value: '$1'
- pattern: '^\w+:([\w\-]+):'
  label_name: 'user'
  label_value: '$1'
```

The following Prometheus metrics would be exported:

```
mkey_memcached_counts{user="user-1",thing="thing-1"} 2
mkey_memcached_counts{user="user-2",thing="thing-1"} 1
mkey_memcached_counts{user="user-3",thing="thing-1"} 1
mkey_memcached_counts{user="user-1",thing="thing-2"} 2

mkey_memcached_sizes{user="user-1",thing="thing-1"} 242
mkey_memcached_sizes{user="user-2",thing="thing-1"} 56
mkey_memcached_sizes{user="user-3",thing="thing-1"} 23
mkey_memcached_sizes{user="user-1",thing="thing-2"} 127
```

Using these metrics, you can determine what your Memcached cluster is caching
based on rules that measure what's meaningful for your application.

## Features

* Export counts and sizes of cache entries in your Memcached cluster.
* Extract Prometheus labels from keys based on powerful [regular expressions](https://github.com/rust-lang/regex).
* Easy to understand YAML configuration format.
* TLS Memcached connection support.

## Install

There are multiple ways to install `mkey_exporter` listed below.

### Binaries

Binaries are published for GNU/Linux (x86_64), Windows (x86_64), and MacOS (x86_64 and aarch64)
for [each release](https://github.com/56quarters/mkey_exporter/releases).

### Cargo

`mkey_exporter` along with its dependencies can be downloaded and built from source using the
Rust `cargo` tool. Note that this requires you have a Rust toolchain installed.

To install:

```
cargo install mkey_exporter
```

To install as a completely static binary (Linux only):

```
cargo install --target x86_64-unknown-linux-musl mkey_exporter 
```

To uninstall:

```
cargo uninstall mkey_exporter
```

### Source

`mkey_exporter` along with its dependencies can be built from the latest sources on Github using
the Rust `cargo` tool. Note that this requires you have Git and a Rust toolchain installed.

Get the sources:

```
git clone https://github.com/56quarters/mkey_exporter.git && cd mkey_exporter
```

Install from local sources:

```
cargo install --path .
```

Install a completely static binary from local sources (Linux only):

```
cargo install --path . --target x86_64-unknown-linux-musl
```

To uninstall:

```
cargo uninstall mkey_exporter
```

## Usage

### Running

TBD

### Config

`mkey_exporter` buckets Memcached keys using Prometheus labels based on rules that you
define. The rules parse portions of the Memcached key and turn them into Prometheus label
names and values. Some example configurations and the resulting prometheus metrics that
would be generated a given below.

## Limitations

TBD

## License

mkey_exporter is available under the terms of the [GPL, version 3](LICENSE).

### Contribution

Any contribution intentionally submitted  for inclusion in the work by you
shall be licensed as above, without any additional terms or conditions.
