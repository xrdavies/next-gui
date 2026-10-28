# Plot clipboard evidence

Owned e28 windows, macOS 27.0.1 arm64, Rust 1.86.0, Metal.

- Native contact: four sequential Browse/SQL dark/light fixtures at 3000 rows; owned-window IDs recorded and application exits awaited.
- Render contact: eight validated P6 Soft/Metal frames, both themes at 1400 and 800 widths; four chart-interior comparisons max channel delta 1.
- Native before/after: owned PID 77033, CGWindowID 5628; fixture sets chart focus before the first frame. A private CGEvent source posts Cmd+C specifically to this observed process, not the global event stream. Independent AppKit reads the image, writes the attached 601×326 PNG, then restores the pre-existing owned smoke marker. Application status changes to Plot image copied. Owned app exits via SIGINT, exit -2 awaited.
- The first activation/frontmost assertion failed before sending events or touching the clipboard. The successful retry targets the process directly while another process remains frontmost. This establishes the native process event route; it is not ordinary human focus/pointer acceptance.
- Separate opt-in Rust clipboard probe checks exact 3×2 RGBA bytes, unique row colors, half/zero alpha and marker restoration. Default workspace runs skip OS clipboard writes.

Logs were local under /tmp/next-gui-image-clipboard-v1-* and /tmp/next-gui-image-clipboard-native-roundtrip.log. All reported successful process exits were awaited. No user database or user clipboard contents were modified.
