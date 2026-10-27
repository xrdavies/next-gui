# Table SQL preview verification

Implementation `5caa315d88e3a777d360df6641f3f08da3328c55`. Actual scenes/readbacks and own-PID native windows.

`NEXT_GUI_DB_BROWSER_TABLE_PREVIEW_QA_OUT=/tmp/next-gui-table-preview-final cargo test --offline --locked -p e28_db_browser table_sql_preview -- --nocapture` exit0. Four Soft/Metal pairs at1400×1000 and800×600, dark/light full-frame max2 each. Both contacts/full light800 inspected. Preview stays outside the scrolling form and above Save/Cancel; at600 height its own scrollbar exposes remaining SQL lines. Focus/selection survives resizing/themes.

Native GPU: `table-preview` dark top and `table-preview-bottom` light bottom. Own-PID CGWindow2101/2104,1400×892 at260,48,1920×1080 scale1, active=false/hidden=false. Both full captures inspected. Helpers0, owned SIGINT children-2 awaited. Logs `/tmp/next-gui-table-preview-{visual,native-dark,native-light}.log`, corresponding native directories retain metadata.

No native pointer/keyboard/IME/clipboard/picker acceptance, original binary or Windows/Linux runtime. Shared widgets/render code unchanged. Selection/copy/read-only/IME and geometry tested by model events.
