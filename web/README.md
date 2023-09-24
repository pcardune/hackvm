# Web Interface

This was last successfully built with [bun](https://bun.sh/) and [vite](https://vitejs.dev/).

## Building

Make sure to build the hackvm wasm package first, following the
directions in [the hackvm readme](../hackvm/README.md).

```bash
bun install
bun run build
```

Files will be written to `dist/`, where you can serve them with your favorite web server:

```bash
python3 -m http.server --directory dist/
```

or with vite itself:

```bash
bun run preview
```

## Developing

```bash
bun run dev
```
