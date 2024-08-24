# streamer-wands-linux

The streamer wands Noita mod uses the
[pollnet](https://github.com/probable-basilisk/pollnet) library to make their
websocket connection.

This .dll sadly implodes on Linux under Proton with the following error:
> TLS error: OS Error -2146762482 (FormatMessageW() returned error 317) (os error -2146762482)

I have not found a library that would work instead to patch streamer wands or
even make a pull request upstream.

So this program is a hacky workraround - we install a tiny additional patch mod
that simply makes streamer wands write the data it wants to send into a file
(thankfully vanilla Lua(JIT) can do that) and then as an outside program we
read the file and send the updates to the websocket manually.

## Build and Run

It's a simple Rust CLI, so `cargo build`/`cargo run` should work.

You might notice that there's a nix flake present, so you can do
`nix run github:necauqua/streamer-wands-linux` and it will just work.

### License
As with most of my things, it's plain MIT, meaning you can use this in most
conceivable ways if you include the LICENSE file which has my name on top of it
