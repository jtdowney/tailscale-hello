#!/bin/sh

/usr/bin/tailscaled --state=/var/lib/tailscale/tailscaled.state --socket=/var/run/tailscale/tailscaled.sock &
/usr/bin/tailscale up --authkey=${TS_AUTHKEY} --hostname=${TS_HOSTNAME}
/usr/bin/tailscale-hello
