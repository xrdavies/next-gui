# CSV batch import evidence

Reviewed code: `83bec7daaef8ae2f71bdb43c0d34eb854695ec05`. Host macOS27.0.1 arm64 / Rust1.86.0.

The in-process `csv_batch_file_controls_keep_targets_sessions_and_four_choice_confirmation`
test prepares isolated CSV files and a real in-memory SQLite connection. Both themes,
1400×1000 and800×600, four phases (files/separate/confirm/remaining) produce16 Soft
frames and16 Metal readbacks; maximum RGB channel delta1–2. All16 pairs passed.
The final24 CSV tests exited0. File names lead each list row; parent paths remain
visible through horizontal scrolling and the selected native path is accessible.

The six native GPU captures use owned fixture files, QA modes csv-batch,
csv-batch-separate and csv-batch-confirm in both themes. The ready marker and a
layer0 onscreen window of at least1400×860 were required before window-ID capture.
Each owned app was stopped by SIGINT (exit−2 awaited), each helper exited0, and the
six-window driver exited0. Fixtures contain `id,name,amount` and Japanese text.
Native screenshots show prepared state, not actual OS import/picker interactions.
Mob screenshot still returned wallpaper only. Native keyboard/pointer/resize/IME/
file-picker acceptance and Windows/Linux/original binary runtime remain not_run.

Sources: `/tmp/next-gui-csv-batch-final-*.ppm`,
`/tmp/next-gui-csv-batch-final-visual.log`,
`/tmp/next-gui-csv-batch-native-{dark,light}-csv-batch*/`,
`/tmp/next-gui-csv-batch-native.log`. Contact sheets and full small light/file and
confirmation/native frames were inspected. Contacts are reduced previews.

SHA-256:

- `soft-contact.png`: `001b31ff688a7049a7f5de1fcf399b065fbf049851e94055b0f0779435949b9a`
- `small-light-files.png`: `560f9d05615d00f35eb22d3e12aa9b14706c4acfeec1c4f447b4f75ad8d743cb`
- `small-light-confirm.png`: `0ce075e54f4cdaa95c5e6bab837f3e8682bac08dae23288fa4f24d38bf81be09`
- `native-contact.png`: `935aaf919c25794e3825462b63c461fd33e6535ad85af877d0aa28c7441b974b`
- `native-light.png`: `aa235d7b7917a7e67312f9f69d0d8d6586f4222eb775fc91e64035123e48f16c`
- `native-dark-confirm.png`: `74bd699ade005e867f3ce700e9a937a28d695dafe3bdbbd8e069f911184efab1`
