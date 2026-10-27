# Capture provenance

- Target: `fdbf1e3c3942071199ded516c45ece7e636c55ac`.
- `header-{dark,light}-soft.png`: e28 `header_context_uses_the_clicked_column_and_supports_keyboard_selection`, 1000 × 860; selected Chinese column, retained first active cell, keyboard-highlighted Copy Column Name; PPM converted with sips.
- `header-native-metal.png`: own e28 CGWindow captured with `NEXT_GUI_GPU_SURFACE=1`, `NEXT_GUI_DB_BROWSER_QA_TAB=header-menu`; the QA branch opens/highlights the menu programmatically. It does not demonstrate native input.
- `office-native-contact.png`: AppKit contact sheet of fresh own-window captures for e10–e15 and e20–e27, native GPU Surface. Static render skim only.
- `contrast-before-soft.png`: intermediate pre-fix screenshot with the old browser selection background and the new hover rendering before `hover_fg` was added. Baseline evidence for the contrast issue, not a released commit or final acceptance capture.
- Primary-display screenshot still shows wallpaper only; no blind input, permission changes, Xcode license acceptance or native OS clipboard claim.

| File | SHA-256 |
|---|---|
| contrast-before-soft.png | `3426a83874a11800bebd903b4008e2355d0c5ace1bf3676158f3885d63aff349` |
| header-dark-soft.png | `ad16f3255c58e426f2b96ad25f4e2bbb09276b432fb3c8bf5fa500fc1c0774fb` |
| header-light-soft.png | `ac7452fb04c7581a0c1c3b1e78e69367034f20d9b9b952c53f29139215702e15` |
| header-native-metal.png | `7f95dea33162a1e70cd610715877b94f1de6396c9f0f708ca1c0e046a6f011c2` |
| office-native-contact.png | `ce31c5bb951d57ce7ac2dd7ef29e24df685e2dd235ce725eb58f970c04331d12` |
