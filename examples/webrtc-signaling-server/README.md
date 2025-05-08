
### public webrtc-signaling-server frontend

In order to generate static website content, first you need build it. This can be done via npm.

```bash
npm install
npm run build
```

These commands will install all dependencies and run [rollup.js](https://rollupjs.org/), which is used for bundling the JavaScript code and dependencies for Code Mirror.

Once the steps above are done, a `./frontent/dist` directory should appear. 