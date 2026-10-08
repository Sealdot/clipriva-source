# Local performance soak test

Use this workflow to observe ClipRiva's resident memory and CPU trend over several days without
adding telemetry, an account, or a network destination. It runs the local release app and keeps
the run artifacts in the repository's ignored `logs/soak/` directory.

## Start a three-day run

Build the unsigned local app bundle first. A development build also includes Vite and development
tooling, so its memory numbers are not representative of the packaged application.

```bash
pnpm tauri build --bundles app --no-sign
scripts/local-soak-test.sh start
```

The sampler records the ClipRiva process tree once per minute and stops the app it launched
after three days. The UI and clipboard capture remain usable during the run. To choose another
duration or sampling interval, pass the binary, whole days, and seconds explicitly:

```bash
scripts/local-soak-test.sh start src-tauri/target/release/bundle/macos/ClipRiva.app 4 120
```

## Inspect or stop the run

```bash
scripts/local-soak-test.sh status
scripts/local-soak-test.sh report
scripts/local-soak-test.sh stop
```

`status` and `report` use the newest run unless a run directory is supplied. A run contains:

- `resources.csv`: timestamp, elapsed time, process count, summed RSS/VSZ, CPU, and app-log size.
- `app.log`: a link to the run's temporary native stderr capture. ClipRiva's error paths must not
  write clipboard contents.
- `monitor.log`: sampler failures only.
- `summary.txt`: first/latest/average/maximum RSS and average/maximum CPU after `report` runs.
- PID, state, and value-free run configuration files used by the control commands.

These files stay local and are ignored by Git. Before sharing any excerpt, inspect it for local
paths or unexpected sensitive values. Do not treat a short run as proof that memory cannot grow;
review the latest, average, and maximum RSS together and correlate increases with app activity.
On macOS, Launch Services and `launchd` require a small copied sampler plus the stderr target under
`/tmp`; neither is sent elsewhere, and the copied sampler removes itself when the run ends.
