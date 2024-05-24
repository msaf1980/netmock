# msaf1980/netmock

A fake stream for testing network applications backed by buffers.

# Usage

```toml
[dependencies]
netmock = "0.1"
```

Next, add this to your crate:

```rust
use netmock::stream::SimpleMockStream;
```

```rust
use netmock::stream::CheckedMockStream;
```

The general idea is to treat `SimpleMockStream` or `CheckedMockStream` as you would `TcpStream`. You can find documentation online at [docs.rs](https://docs.rs/netmock/).

# License

`netmock` is primarily distributed under the terms of both the MIT license.

See LICENSE-MIT for details.
