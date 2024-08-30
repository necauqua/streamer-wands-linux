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

When you run it, it will install the patch mod into your Noita Steam
installation, you'll have to enable it. You can read the source of the
`patch-mod/` yourself, it just makes the socket thing streamer wands uses
write the data to a file - interacting with the filesystem making the patch mod
itself an Unsafe Mod sadly.

After that, it just works - note that it immediately connects to the websocket
without any regard to if the game is running, but it only sends messages when
the file is written to by the patch mod (except pings).

> [!WARNING]
> Also note that it does read the token from your streamer wands install to
> connect to said websocket - I mean it's obvious it has to do that, but you've
> been warned - you can read the Rust code (sadly it's a bit harder than Lua to
> read) to make sure nothing shady happens, or ask a developer you trust to do
> that.

### License
As with most of my things, it's plain MIT, meaning you can use this in most
conceivable ways if you include the LICENSE file which has my name on top of it
