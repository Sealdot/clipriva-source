#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROJECT_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
SOAK_ROOT="$PROJECT_ROOT/logs/soak"

usage() {
  cat <<'EOF'
Usage:
  scripts/local-soak-test.sh start [app-or-binary] [days] [sample-seconds]
  scripts/local-soak-test.sh status [run-directory]
  scripts/local-soak-test.sh report [run-directory]
  scripts/local-soak-test.sh stop [run-directory]

Defaults:
  app-or-binary   src-tauri/target/release/bundle/macos/ClipRiva.app
  days            3
  sample-seconds  60

The start command launches only the supplied local app, samples its process
tree, and stops that launched process when the requested duration elapses.
Generated output stays under the ignored logs/soak directory.
EOF
}

latest_run_directory() {
  if [ ! -d "$SOAK_ROOT" ]; then
    return 1
  fi
  find "$SOAK_ROOT" -mindepth 1 -maxdepth 1 -type d -print 2>/dev/null \
    | LC_ALL=C sort \
    | tail -n 1
}

resolve_run_directory() {
  requested=${1:-}
  if [ -n "$requested" ]; then
    case "$requested" in
      /*) run_directory=$requested ;;
      *) run_directory="$PROJECT_ROOT/$requested" ;;
    esac
  else
    run_directory=$(latest_run_directory || true)
  fi
  if [ -z "${run_directory:-}" ] || [ ! -d "$run_directory" ]; then
    echo "No local soak run was found." >&2
    exit 1
  fi
  printf '%s\n' "$run_directory"
}

read_pid() {
  pid_file=$1
  if [ ! -f "$pid_file" ]; then
    return 1
  fi
  pid=$(sed -n '1p' "$pid_file")
  case "$pid" in
    ''|*[!0-9]*) return 1 ;;
  esac
  printf '%s\n' "$pid"
}

process_tree_ids() {
  root_pid=$1
  all_ids=$root_pid
  pending_ids=$root_pid
  while [ -n "$pending_ids" ]; do
    next_ids=""
    for parent_pid in $pending_ids; do
      child_ids=$(pgrep -P "$parent_pid" 2>/dev/null || true)
      if [ -n "$child_ids" ]; then
        all_ids="$all_ids $child_ids"
        next_ids="$next_ids $child_ids"
      fi
    done
    pending_ids=$next_ids
  done
  printf '%s\n' "$all_ids" | tr '\n' ' '
}

application_pid() {
  executable_path=$1
  ps -axo pid=,command= | awk -v target="$executable_path" '
    {
      pid = $1
      $1 = ""
      sub(/^ +/, "")
      if ($0 == target) print pid
    }
  ' | tail -n 1
}

sample_process() {
  run_directory=$1
  app_pid=$2
  duration_seconds=$3
  sample_seconds=$4
  app_log_source=${5:-}
  monitor_program=${6:-}
  resource_log="$run_directory/resources.csv"
  app_log="$run_directory/app.log"
  started_epoch=$(date +%s)

  printf '%s\n' \
    "timestamp,elapsed_seconds,process_count,rss_kb,vsz_kb,cpu_percent,app_log_bytes" \
    > "$resource_log"
  printf '%s\n' "running" > "$run_directory/state"

  while kill -0 "$app_pid" 2>/dev/null; do
    now_epoch=$(date +%s)
    elapsed_seconds=$((now_epoch - started_epoch))
    process_ids=$(process_tree_ids "$app_pid")
    pid_csv=$(printf '%s' "$process_ids" | tr -s ' ' ',' | sed 's/^,//; s/,$//')
    metrics=$(ps -p "$pid_csv" -o rss=,vsz=,%cpu= 2>/dev/null \
      | awk '{ rss += $1; vsz += $2; cpu += $3; count += 1 }
             END { printf "%d %d %.2f %d", rss, vsz, cpu, count }')
    set -- $metrics
    rss_kb=${1:-0}
    vsz_kb=${2:-0}
    cpu_percent=${3:-0}
    process_count=${4:-0}
    app_log_bytes=$(wc -c < "$app_log" | tr -d ' ')
    timestamp=$(date '+%Y-%m-%dT%H:%M:%S%z')
    printf '%s,%s,%s,%s,%s,%s,%s\n' \
      "$timestamp" "$elapsed_seconds" "$process_count" "$rss_kb" "$vsz_kb" \
      "$cpu_percent" "$app_log_bytes" >> "$resource_log"

    if [ "$elapsed_seconds" -ge "$duration_seconds" ]; then
      printf '%s\n' "duration-complete" > "$run_directory/state"
      kill "$app_pid" 2>/dev/null || true
      if [ -n "$monitor_program" ] && [ -f "$monitor_program" ]; then
        unlink "$monitor_program"
      fi
      exit 0
    fi
    sleep "$sample_seconds"
  done

  if [ -n "$monitor_program" ] && [ -f "$monitor_program" ]; then
    unlink "$monitor_program"
  fi
  printf '%s\n' "app-exited" > "$run_directory/state"
}

start_run() {
  launch_target=${1:-src-tauri/target/release/bundle/macos/ClipRiva.app}
  days=${2:-3}
  sample_seconds=${3:-60}
  case "$days" in
    ''|*[!0-9]*) echo "days must be a positive integer" >&2; exit 1 ;;
  esac
  case "$sample_seconds" in
    ''|*[!0-9]*) echo "sample-seconds must be a positive integer" >&2; exit 1 ;;
  esac
  if [ "$days" -lt 1 ] || [ "$sample_seconds" -lt 1 ]; then
    echo "days and sample-seconds must be positive integers" >&2
    exit 1
  fi

  case "$launch_target" in
    /*) launch_path=$launch_target ;;
    *) launch_path="$PROJECT_ROOT/$launch_target" ;;
  esac
  case "$launch_path" in
    *.app)
      executable_path="$launch_path/Contents/MacOS/clipriva"
      if [ ! -d "$launch_path" ] || [ ! -x "$executable_path" ]; then
        echo "Local app bundle is missing or incomplete: $launch_target" >&2
        echo "Build it first with: pnpm tauri build --bundles app --no-sign" >&2
        exit 1
      fi
      ;;
    *)
      executable_path=$launch_path
      if [ ! -x "$executable_path" ]; then
        echo "Local release binary is missing or not executable: $launch_target" >&2
        exit 1
      fi
      ;;
  esac
  if [ -n "$(application_pid "$executable_path")" ]; then
    echo "ClipRiva is already running from this launch target. Quit it before starting a soak run." >&2
    exit 1
  fi

  mkdir -p "$SOAK_ROOT"
  run_id=$(date '+%Y%m%d-%H%M%S')
  run_directory="$SOAK_ROOT/$run_id"
  mkdir "$run_directory"
  duration_seconds=$((days * 24 * 60 * 60))

  {
    printf 'started_at=%s\n' "$(date '+%Y-%m-%dT%H:%M:%S%z')"
    printf 'duration_days=%s\n' "$days"
    printf 'sample_seconds=%s\n' "$sample_seconds"
    printf 'launch_target=%s\n' "${launch_path#"$PROJECT_ROOT"/}"
  } > "$run_directory/run.conf"

  cd "$PROJECT_ROOT"
  if [ -d "$launch_path" ]; then
    app_log_source="/tmp/clipriva-soak-$run_id.stderr.log"
    : > "$app_log_source"
    printf '%s\n' "$app_log_source" > "$run_directory/app-log-source"
    ln -s "$app_log_source" "$run_directory/app.log"
    open -n --stdout /dev/null --stderr "$app_log_source" "$launch_path"
    app_pid=""
    attempt=0
    while [ -z "$app_pid" ] && [ "$attempt" -lt 10 ]; do
      sleep 1
      app_pid=$(application_pid "$executable_path")
      attempt=$((attempt + 1))
    done
    if [ -z "$app_pid" ]; then
      printf '%s\n' "start-failed" > "$run_directory/state"
      echo "Launch Services did not return a running ClipRiva process." >&2
      exit 1
    fi
  else
    app_log_source=""
    nohup "$executable_path" >> "$run_directory/app.log" 2>&1 < /dev/null &
    app_pid=$!
  fi
  printf '%s\n' "$app_pid" > "$run_directory/app.pid"
  sleep 2
  if ! kill -0 "$app_pid" 2>/dev/null; then
    printf '%s\n' "start-failed" > "$run_directory/state"
    echo "ClipRiva exited during startup. Inspect $run_directory/app.log" >&2
    exit 1
  fi

  monitor_label="com.clipriva.soak.$run_id"
  printf '%s\n' "$monitor_label" > "$run_directory/monitor.label"
  monitor_program="/tmp/clipriva-soak-monitor-$run_id.sh"
  cp "$SCRIPT_DIR/local-soak-test.sh" "$monitor_program"
  chmod +x "$monitor_program"
  printf '%s\n' "$monitor_program" > "$run_directory/monitor.program"
  launchctl submit -l "$monitor_label" \
    -o "$run_directory/monitor.log" -e "$run_directory/monitor.log" -- \
    "$monitor_program" __sample \
    "$run_directory" "$app_pid" "$duration_seconds" "$sample_seconds" "$app_log_source" \
    "$monitor_program"
  sleep 1
  monitor_pid=$(launchctl print "gui/$(id -u)/$monitor_label" 2>/dev/null \
    | awk '/pid = / { print $3; exit }')
  if [ -z "$monitor_pid" ]; then
    echo "The local resource sampler did not start." >&2
    kill "$app_pid" 2>/dev/null || true
    exit 1
  fi
  printf '%s\n' "$monitor_pid" > "$run_directory/monitor.pid"

  echo "Started a ${days}-day local ClipRiva soak run."
  echo "Run directory: $run_directory"
  echo "App PID: $app_pid; monitor PID: $monitor_pid"
}

show_status() {
  run_directory=$(resolve_run_directory "${1:-}")
  state=$(sed -n '1p' "$run_directory/state" 2>/dev/null || printf 'starting')
  app_pid=$(read_pid "$run_directory/app.pid" || true)
  monitor_pid=$(read_pid "$run_directory/monitor.pid" || true)
  app_status=stopped
  monitor_status=stopped
  if [ -n "$app_pid" ] && kill -0 "$app_pid" 2>/dev/null; then app_status=running; fi
  if [ -n "$monitor_pid" ] && kill -0 "$monitor_pid" 2>/dev/null; then monitor_status=running; fi
  echo "Run directory: $run_directory"
  echo "State: $state; app: $app_status; monitor: $monitor_status"
  if [ -f "$run_directory/resources.csv" ]; then
    tail -n 1 "$run_directory/resources.csv"
  fi
}

report_run() {
  run_directory=$(resolve_run_directory "${1:-}")
  resource_log="$run_directory/resources.csv"
  if [ ! -s "$resource_log" ]; then
    echo "No resource samples are available yet." >&2
    exit 1
  fi
  awk -F, '
    NR == 2 { first_ts = $1; first_rss = $4 }
    NR > 1 {
      samples += 1
      last_ts = $1
      last_rss = $4
      rss_sum += $4
      cpu_sum += $6
      if ($4 > rss_max) rss_max = $4
      if ($6 > cpu_max) cpu_max = $6
    }
    END {
      if (samples == 0) { print "No resource samples are available yet."; exit 1 }
      printf "samples=%d\n", samples
      printf "period=%s to %s\n", first_ts, last_ts
      printf "rss_mib_first=%.1f\n", first_rss / 1024
      printf "rss_mib_latest=%.1f\n", last_rss / 1024
      printf "rss_mib_average=%.1f\n", rss_sum / samples / 1024
      printf "rss_mib_max=%.1f\n", rss_max / 1024
      printf "cpu_percent_average=%.2f\n", cpu_sum / samples
      printf "cpu_percent_max=%.2f\n", cpu_max
    }
  ' "$resource_log" | tee "$run_directory/summary.txt"
}

stop_run() {
  run_directory=$(resolve_run_directory "${1:-}")
  app_pid=$(read_pid "$run_directory/app.pid" || true)
  monitor_pid=$(read_pid "$run_directory/monitor.pid" || true)
  monitor_label=$(sed -n '1p' "$run_directory/monitor.label" 2>/dev/null || true)
  if [ -n "$monitor_label" ]; then
    launchctl remove "$monitor_label" 2>/dev/null || true
  elif [ -n "$monitor_pid" ] && kill -0 "$monitor_pid" 2>/dev/null; then
    kill "$monitor_pid"
  fi
  if [ -n "$app_pid" ] && kill -0 "$app_pid" 2>/dev/null; then
    kill "$app_pid"
  fi
  monitor_program=$(sed -n '1p' "$run_directory/monitor.program" 2>/dev/null || true)
  if [ -n "$monitor_program" ] && [ -f "$monitor_program" ]; then
    unlink "$monitor_program"
  fi
  printf '%s\n' "stopped" > "$run_directory/state"
  echo "Stopped the local soak run in $run_directory"
}

command=${1:-}
case "$command" in
  start) shift; start_run "$@" ;;
  status) shift; show_status "$@" ;;
  report) shift; report_run "$@" ;;
  stop) shift; stop_run "$@" ;;
  __sample) shift; sample_process "$@" ;;
  *) usage; exit 1 ;;
esac
