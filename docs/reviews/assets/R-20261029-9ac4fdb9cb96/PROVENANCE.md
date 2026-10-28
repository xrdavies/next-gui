# Capture provenance

Final source: `9ac4fdb9cb96eeddec85f6ffeb35d0ca3509aa42`. Host macOS27.0.1/arm64, actual Metal.

- Final required checks:83601 exit0;670 tests/43 groups, strict Clippy, workspace build, host macOS+Windows features.
- UI driver:48069 exit0. Six sequential owned e28 windows: current Browse, current SQL, imported native Plot settings, each dark/light. Actual1400×892 windows, captured by owned PID/CGWindowID. Helpers exit0; owned applications deliberately stopped with SIGINT(-2) and awaited. No concurrent native drivers.
- Native fixture uses canonical DB4S plot_x_axis/plot_y_axes; blue StepLeft+Disc and gold Impulse+None independently rendered. Final selectors show the selected gold series' Impulse/No marker. The first native capture exposed stale global selector labels; those earlier images are not these final artifacts.
- Eight generated P6 frames from current Browse/SQL × dark/light ×1400×1000/800×600. All payload lengths checked after writer exit. Actual Soft/Metal visible plot-interior max channel delta1 in all eight,293760 wide/53136 narrow pixels. Edges and the whole window are outside this comparison.
- Existing integration fixture intentionally retains an informational cell-draft status in render frames after its draft-rejection test. This is not an unhandled load error; native fixtures are clean.
- All contact frames and the full imported native window were inspected. Native OS keys/IME/pickers and original Plot dock side-by-side remain unverified; primary Mob capture omits windows. Office shared widgets were unchanged in this slice.

SHA256:

- `native-contact.png`: `d45410450e49df3665af9c9c00be63a560a3383fa586c5e13f3d0cdfcbc03776`
- `render-contact.png`: `2c3196ce1631bae3da52448344b45388ccd5cea7da29d425e6e59b97d176ddaa`
- `native-imported.png`: `f77b2648e123ac62bbfb0bc6ae7f510d4a75b7cc768591b8970c42aa96239221`
