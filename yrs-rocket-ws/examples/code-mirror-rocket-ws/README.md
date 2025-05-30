
### Running an example

In order to generate static website content, first you need build it. This can be done via npm.

```bash
cd ../examples/code-mirror/frontend
npm install
npm run build
```

These commands will install all dependencies and run [rollup.js](https://rollupjs.org/), which is used for bundling the JavaScript code and dependencies for Code Mirror.

Once the steps above are done, a `dist` directory should appear. If so, all you need to do is to run following command from the */yrs-rocket-ws* or *main git repository* directory:

```bash
cargo run --example code-mirror-rocket-ws
```

It will run a local warp server with an index page at [http://localhost:8000](http://localhost:8000).