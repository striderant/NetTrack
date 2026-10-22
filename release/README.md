# NetTrack

Real-time network monitoring TUI for Windows. See which application talks to
which IP address and port, over TCP and UDP, with live byte rates, process
attribution, hostnames, and a country for every remote peer.

![Connections table](docs/screenshot.png)

## What it shows

For every active socket:

| Column | Meaning |
|--------|---------|
| Proto | TCP or UDP |
| Local | Local address and port |
| Remote | Remote address and port (`*:*` for UDP bound sockets) |
| State | TCP connection state (ESTABLISHED, LISTEN, TIME_WAIT, …) |
| PID | Owning process ID |
| Process | Process name |
| ↑ Rate / ↓ Rate | Live send/receive byte rate |
| Geo | Country flag + code for the remote peer |

A `▶` marker before `UDP` means the peer was learned from live traffic (ETW),
because Windows does not record a remote endpoint for connectionless UDP
sockets in the socket table.

## Tabs

- **Connections** — the main table, sortable and filterable.
- **Processes** — per-process bandwidth and connection totals.
- **Interfaces** — per-NIC traffic, packets, errors, and addresses.
- **Routing** — the IPv4/IPv6 routing table.
- **Alerts** — new listeners, new countries reached, bandwidth spikes, and
  repeated connection failures.

## Controls

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Tab` / `Shift+Tab` | Switch tabs |
| `1`–`5` | Jump directly to a tab |
| `j` / `k` / `↑` / `↓` | Move up/down |
| `J` / `K` | Page down/up |
| `g` / `G` | First/last row |
| `Enter` | Connection details |
| `/` | Filter |
| `Esc` | Clear filter |
| `s` | Change sort column |
| `Space` | Pause/resume updates |
| `r` | Refresh now |
| `?` | Help |

### Filter language

Filters are combined with an implicit AND. Bare words match anywhere in the row.

```
proto:tcp state:estab port:443 process:chrome host:github
country:us local:192.168. service:quic flow:true listen:true ipv6:true
```

## Running

Download `nettrack.exe` and run it. It will ask for administrator rights on
first launch (via the standard UAC prompt). Administrator rights are required
to:

- Enable the ETW kernel network providers, which reveal the remote peer of a
  UDP socket.
- Read per-connection TCP statistics for processes owned by other users.

If you run without elevation, the tool still works but shows only local socket
information.

### Headless mode (scripting)

Print a single snapshot and exit instead of opening the interface:

```powershell
.\nettrack.exe --headless
.\nettrack.exe --headless --json
.\nettrack.exe --headless -n 5      # wait for 5 samples (better rates)
```

JSON output is handy for piping into other tools:

```powershell
.\nettrack.exe --headless --json | ConvertFrom-Json | Where-Object { $_.protocol -eq 'TCP' }
```

## Building from source

Requires Rust and a MinGW-w64 toolchain (the bundled MSVC toolchain works too).

```powershell
cargo build --release
```

The release binary is fully static: no external DLLs beyond standard Windows
system libraries, so it runs as a single portable `.exe`.

GeoLite2 country data is downloaded automatically at build time. Set
`NETTRACK_SKIP_GEOIP=1` to skip it (country column will be empty).

## Data sources

| Source | Used for |
|--------|----------|
| `GetExtendedTcpTable` / `GetExtendedUdpTable` | Socket inventory, process IDs |
| ETW `Microsoft-Windows-Kernel-Network` | UDP peers, connect/disconnect events |
| `GetPerTcpConnectionEStats` | Per-connection byte counts |
| `ipconfig /displaydns` | Hostnames (no network traffic of its own) |
| MaxMind GeoLite2-Country | Remote peer country |
| `GetIfTable2` / `GetIpForwardTable2` / `GetIpNetTable2` | Interfaces, routes, neighbours |

## License

MIT OR Apache-2.0.
