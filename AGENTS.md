# AGENTS.md - linux-phone-oco/lab-tools

**Owner**: `orchestration` - the linux-phone-oco orchestration role, whichever session holds it (signing as `orchestration.RUN`), keeps this repository. The lab-tools builder it launches lands here, one backlog item per run; any other session sends the change to the owner - a message, or a docs backlog row with `--project lab-tools` - and does not land here itself.

The lpoco family (lpoco, lpoco-lab, lpoco-health, lpoco-host, lpoco-bringup, lpoco-tone) and what each does: [`README.md`](README.md). Its backlog, decisions and research live in [linux-phone-oco/docs](https://github.com/linux-phone-oco/docs): `tad q backlog --project lab-tools`.
