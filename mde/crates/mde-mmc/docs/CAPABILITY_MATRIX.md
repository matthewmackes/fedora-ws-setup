# MDE MMC capability matrix (W460 / W498)

Maps Windows Computer Management / Cockpit tasks to `mde-mmc` snap-ins.
Status: `done` = writable path exists · `partial` = inventory or subset · `todo` = backlog.

## Computer Management

| Windows task | Snap-in | Status | Notes |
|--------------|---------|--------|-------|
| Open Computer Management | `compmgmt` | done | `mde-mmc compmgmt.msc` |
| Device Manager view | `devmgmt` | partial | PCI/USB; Python bridge for depth |
| Disable / enable device | `devmgmt` | done | helper + confirm |
| Update / roll back driver | `devmgmt` | todo | W463 |
| Disk list / volumes | `diskmgmt` | partial | lsblk; graphical map todo |
| Format / resize | `diskmgmt` | todo | UDisks mutate |
| LVM / RAID / LUKS / iSCSI | `diskmgmt` | partial | inventory nodes |
| Services list | `services` | done | |
| Start / stop / restart | `services` | done | elevated |
| Startup type Automatic/Manual/Disabled | `services` | done | Masked distinct |
| Dependencies / recovery | `services` | todo | W467 |
| Event Viewer logs | `eventvwr` | done | |
| Clear log | `eventvwr` | done | confirm CLEAR |
| Custom views / follow | `eventvwr` | partial | |
| SELinux AVC | `eventvwr` | done | |
| Remote journal | `eventvwr` | todo | W476 agent |
| Task Scheduler timers | `taskschd` | done | |
| Create Basic Task | `taskschd` | todo | |
| Cron library | `taskschd` | partial | inventory |
| Local users / groups | `lusrmgr` | partial | list + aging |
| Domain / FreeIPA join | `lusrmgr` | todo | |
| Performance counters | `perfmon` | partial | live sample |
| Data collector sets | `perfmon` | partial | scaffold |

## Linux / Cockpit parity

| Domain | Snap-in | Status |
|--------|---------|--------|
| NetworkManager | `network` | partial |
| firewalld | `firewall` | partial |
| Packages / dnf | `packages` | partial |
| SELinux admin | `selinux` | scaffold |
| Containers | `containers` | scaffold |
| Virtualization | `virtualization` | scaffold |
| Certificates | `certificates` | scaffold |
| Subscriptions | `subscriptions` | scaffold |

## Automated tests

```bash
cargo test -p mde-mmc
```

Current coverage: builtin console resolve, presets, JSON roundtrip, services startup map.
Mutation-path integration tests (helper dry-run) are W460 follow-up.
