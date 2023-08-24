# tailscale-hello

This is a simple application that can be deployed on your Tailscale tailnet to test connectivity. When you visit http://hello you will be redirected and presented with a page that welcomes you to Tailscale.

## Run on fly.io

1. Create and log in to your fly.io account
2. Launch a new fly app
   ```sh
   flyctl launch --copy-config --no-deploy
   ```
3. Set the Tailscale auth key
   ```sh
   flyctl secrets set TS_AUTHKEY=<your authkey>
   ```
4. Deploy
   ```sh
   flyctl deploy
   ```

## Contributing

The CSS is using tailwind and requires nodejs to build a new CSS output. During development, you can run `npm run watch` to watch for changes and rebuild the CSS. The css is checked in so you don't need to do this to run the app. There is a Procfile that runs both CSS and the application in a watch mode to speed up development.