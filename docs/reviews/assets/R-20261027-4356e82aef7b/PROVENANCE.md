# Capture provenance

- macOS 27.0.1 arm64; tested code `4356e82aef7bf3da0edd41419bca10b600a980f0`.
- Soft: existing e28 page selection/rebuild regression, QA_OUT, Chinese 80-line SQL; confirms session rendering after the shared editing-state change. Desired-column cases are tested in-process, not inferred from this screenshot.
- Native Metal: owned e28 CGWindow, GPU_SURFACE=1, QA_TAB=sql-long without QA exit flag, ordinary sample SQL static painting. Office sheet includes all fourteen e10–e15/e20–e27 windows, captured and skimmed after final build.
- Primary display still wallpaper; native keys/pointer/IME/clipboard, original DB4S binary and Windows/Linux runtime not exercised.

| Asset | SHA-256 |
|---|---|
| `page-session-soft.png` | `cad425b21c11fff82d509f086054f5fc40448b51b316bf1ca83a121cccc2d1ce` |
| `sql-native-metal.png` | `d28f76d212bc6f58eee7345d1afc7565111397ecb7263d9a36ab664557b5089a` |
| `office-native-contact.png` | `a5d805ed20859b2390d06c2a941070904d9441c19100054a271a058ec9ecabaf` |
