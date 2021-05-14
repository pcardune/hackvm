# HackVM

This repository contains a reimplementation of the hack virtual machine described
in the [nand2tetris online class](https://www.nand2tetris.org/) which runs in a
web browser.

## Directory Layout

- **hackvm** - contains the virtual machine code, which is implemented in Rust and compiles to WebAssembly
- **web** - contains a web frontend for the virtual machine along with some demo programs,
  implemented with Typescript and React.

## Building

There are two parts to the build. First we must compile the rust code into web assembly.

From the `hackvm` directory run:

```bash
wasm-pack build
```

Next we must compile all the Typescript/React code into plain html/javascript.

From the `web` directory run:

```bash
yarn build
```

There should now be a `web/build` which can be published to any static file hosting site. To
view the contents in a web browser, you'll need to actually serve the files via a real http
server (browsers can't load Web Assembly binaries from the local file system for security reasons):

From the `web/build` directory:

```bash
python3 -m http.server
```

For more details about building the website or the vm, refer to the README files in the `hackvm` and `web` subdirectories.
