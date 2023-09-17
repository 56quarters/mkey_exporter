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

Each rule in an `mkey_exporter` parses a value for a particular Prometheus label from a
Memcached key. Rules are evaluated in order. The first rule that sets a value for a particular
label name "wins", no other rules that set that label name will be evaluated for a Memcached
key.

The configuration format is defined as:

```yaml
name: example                  # name of this configuration, used for diagnostics
rules:                         # array of rules to apply, in order, for each Memcached key
- pattern: '^(\w+):'           # regular expression to apply to the Memcached key
  label_name: 'store'          # name of the label to emit, this may NOT contain regular expression captures
  label_value: '$1'            # value of the label to emit, this MAY contain regular expression captures
- pattern: '^\w+:([\w\-]+):'   # you may include as many rules as you want, they will be evaluated in order
  label_name: 'user'
  label_value: '$1'
```

#### Examples

In the following examples, only the `mkey_memcached_counts` metric is shown for brevity.

---

This configuration sets two labels for metrics based on the Memcached keys.

Keys:

```
user-profile:user-1:backup
user-profile:user-1:latest
user-profile:user-2:latest
user-profile:user-3:latest
user-cart:user-1:latest
```

Rules:

```yaml
name: example
rules:
- pattern: '^(\w+):'
  label_name: 'store'
  label_value: '$1'
- pattern: '^\w+:([\w\-]+):'
  label_name: 'user'
  label_value: '$1'
```

Metrics:

```
mkey_memcached_counts{user="user-1",store="user-profile"} 2
mkey_memcached_counts{user="user-2",store="user-profile"} 1
mkey_memcached_counts{user="user-3",store="user-profile"} 1
mkey_memcached_counts{user="user-1",store="user-cart"} 1
```

---

This configuration sets two labels for metrics based on the Memcached keys using
multiple rules to set values for the "store" label.

Keys:

```
up:user-1:backup
up:user-1:latest
up:user-2:latest
up:user-3:latest
uc:user-1:latest
uu:user-1:latest
```

Rules:

```yaml
name: example
rules:
- pattern: '^up:'
  label_name: 'store'
  label_value: 'user-profile'
- pattern: '^uc:'
  label_name: 'store'
  label_value: 'user-cart'
- pattern: '^\w+:'
  label_name: 'store'
  label_value: 'unknown'
- pattern: '^\w+:([\w\-]+):'
  label_name: 'user'
  label_value: '$1'
```

Metrics:

```
mkey_memcached_counts{user="user-1",store="user-profile"} 2
mkey_memcached_counts{user="user-2",store="user-profile"} 1
mkey_memcached_counts{user="user-3",store="user-profile"} 1
mkey_memcached_counts{user="user-1",store="user-cart"} 1
mkey_memcached_counts{user="user-1",store="unknown"} 1
```

---

## Limitations

Every evaluation loop, `mkey_exporter` gets a complete list of keys from the Memcached
server. This means the time taken for each update will increase based on the number of
keys in the server. I've tested up to 3.5M keys running on a server local to the 
`mkey_exporter` process with decent results (~5s update time).

## License

mkey_exporter is available under the terms of the [GPL, version 3](LICENSE).

### Contribution

Any contribution intentionally submitted  for inclusion in the work by you
shall be licensed as above, without any additional terms or conditions.
