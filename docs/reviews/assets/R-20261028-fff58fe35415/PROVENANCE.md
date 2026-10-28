# SQL export visual evidence

Reviewed implementation: `fff58fe35415e10068eda0007ac09493c27af18f`.

- Final SQL module run77691: seven tests passed,16 Soft/Metal frame pairs,
  independently compared max byte difference1–2. Four states (options/data-only/
  exporting/canceling), dark/light at1400×1000 and800×600. Both contact sheets
  and full small options/native progress were opened and inspected.
- Production worker path module run26589: seven tests passed after removing the
  obsolete synchronous application export method; runtime layout unchanged.
- Final native driver90947 exited0, awaited. It built e28 first and captured
  eight owned GPU windows: both themes × exportsql/sql-export-data/
  sql-export-progress/sql-export-canceling. Each helper exited0; each owned
  app was stopped with SIGINT (exit−2) after capture. Native windows1400×892,
  display scale1. Screenshots were captured by owned CGWindowID.
- Initial timed test gate was replaced by explicit channel release: slow concurrent
  rendering could exceed its artificial30-second limit. Final runs have no callback
  panic. This does not change production cooperative cancellation.
- Mob desktop capture showed wallpaper without listed applications; OS input,
  native picker/IME and original binary acceptance were not run. Native window
  captures do not establish those interactions.
- Shared framework/Office code did not change in this slice; Office windows were
  not repeated. Native title/column geometry reflect the final runtime code;
  subsequent edits only removed an unused cfg(test) helper/formatted equivalent SQL.

Raw local artifact SHA-256 (not checked in):

```text
c0861305f668565314c546fcaf42479cfbce3e7817da668c1643d8054f554a26  /tmp/next-gui-sql-export-final-render-dark-1400-sql-export-canceling-metal.ppm
8f80f68fee748c3316abd8242524eb69b24a9993caa9354e825ea70739598a44  /tmp/next-gui-sql-export-final-render-dark-1400-sql-export-canceling.ppm
131d5122a33afa00e8656e899c02512a173d4f8b2b693821cc99f2c9b32ae81f  /tmp/next-gui-sql-export-final-render-dark-1400-sql-export-data-metal.ppm
62410c0110796dd264860d52ad147c5e086f84469a90fec68b1892554c388287  /tmp/next-gui-sql-export-final-render-dark-1400-sql-export-data.ppm
34e74497b57a6d7440058800e391d12e11971558243514e7d76549bef436f808  /tmp/next-gui-sql-export-final-render-dark-1400-sql-export-options-metal.ppm
208ff0f7a2257f95311f50245691123c73e529d11b5587258b88da209a52a139  /tmp/next-gui-sql-export-final-render-dark-1400-sql-export-options.ppm
e9541d34b3993d42d4c0e2fb03b49b86da6d97bff9f32b510422a7afbdb54586  /tmp/next-gui-sql-export-final-render-dark-1400-sql-export-progress-metal.ppm
236fc0b535175788d14986fce98683304c13da208add47fd65616fd77eb3b69b  /tmp/next-gui-sql-export-final-render-dark-1400-sql-export-progress.ppm
0535813a66e3a13b84fcae84aadc444368f68106fde1075c50c937daa97bd59e  /tmp/next-gui-sql-export-final-render-dark-800-sql-export-canceling-metal.ppm
b1c89548041997271890efc29bb7c51e41b8e477cb3c0dd0cf3c4a2a66937ddc  /tmp/next-gui-sql-export-final-render-dark-800-sql-export-canceling.ppm
d738b010564ccd44af5786171ed5890ae99ae502d07e73ec90ddf45c55bbd9eb  /tmp/next-gui-sql-export-final-render-dark-800-sql-export-data-metal.ppm
0b58049e72bc67c17903aa59018bff09765762cca6619bdebbfc4a43b10968d7  /tmp/next-gui-sql-export-final-render-dark-800-sql-export-data.ppm
400933f0a4068bfa513cd55767d26cff0d74b40fdf5c5a9e6943191c676287ce  /tmp/next-gui-sql-export-final-render-dark-800-sql-export-options-metal.ppm
440463ed5aa7ceabc4b47ff95f9eb4b115e37390304cb3b687cc59dec0d37e74  /tmp/next-gui-sql-export-final-render-dark-800-sql-export-options.ppm
4a4bd14703071b23e9b7a41be22e3ac5a408b9933c9006c4b00beb44db720ff8  /tmp/next-gui-sql-export-final-render-dark-800-sql-export-progress-metal.ppm
edd5fb63b369232b51103dc6b03b74933bfbe6968a97020f44d8edd623129952  /tmp/next-gui-sql-export-final-render-dark-800-sql-export-progress.ppm
90bc0cecd9e2bf8f2801c13dcd356b16d592f7109dadc4b5e66d80d039feb3d2  /tmp/next-gui-sql-export-final-render-light-1400-sql-export-canceling-metal.ppm
d4b2236ef58ad356a66791aa1648404c01701472eb1c38234006cef482899ef4  /tmp/next-gui-sql-export-final-render-light-1400-sql-export-canceling.ppm
c6696232558f47bbfa14e6c2f41fbf50df49121986d1dc9e15ab89eacd799827  /tmp/next-gui-sql-export-final-render-light-1400-sql-export-data-metal.ppm
1743cbbdcbfd681e2deb60cbca0720112a5c3a68f06030491e743c4bf86717bb  /tmp/next-gui-sql-export-final-render-light-1400-sql-export-data.ppm
df1a6316baae064447763627d539fc95cb3e3997e76226c267051fd2ec21228c  /tmp/next-gui-sql-export-final-render-light-1400-sql-export-options-metal.ppm
dd4a9c817f261f8c8136b1644970683558a7b273c9e18893587c5608e17b4864  /tmp/next-gui-sql-export-final-render-light-1400-sql-export-options.ppm
caeee5d10c4beb09ad6b3ba5285f7e8edbfd7a415a0f03e4d480e9df06083fca  /tmp/next-gui-sql-export-final-render-light-1400-sql-export-progress-metal.ppm
a7c3168ece88f69e148f781ac915e3ac14dada94884b920f4152e428c2f2b862  /tmp/next-gui-sql-export-final-render-light-1400-sql-export-progress.ppm
b790859911292b199891aaa29fc5ffb9d73be7f9a7b2689454460d6afa66bb9f  /tmp/next-gui-sql-export-final-render-light-800-sql-export-canceling-metal.ppm
93b476279dfb84fc3df21418ea813458cd5c29185edfb6e323dcbb6732cdcb9c  /tmp/next-gui-sql-export-final-render-light-800-sql-export-canceling.ppm
4874af035000c5d4a62c23ae8f6c5b19c06b0e41b11be7bb80b3a16c2b5031e2  /tmp/next-gui-sql-export-final-render-light-800-sql-export-data-metal.ppm
7ab91abfcb2499ee94843d08d189476793d4e597bfc6865d1d772b8e667cc19f  /tmp/next-gui-sql-export-final-render-light-800-sql-export-data.ppm
8026111aec7065ec253578f086904f86dd1ae08f3cbbf0dc874c247b7cf9afa1  /tmp/next-gui-sql-export-final-render-light-800-sql-export-options-metal.ppm
ebbd4be36b74a09781f33dc4463977de3495fa1e32384c57b61d2d030c33617e  /tmp/next-gui-sql-export-final-render-light-800-sql-export-options.ppm
a384838f72f2b426f8a3adcd04bdbeed9a65eebf1ee1dd865aec0b1094af03b1  /tmp/next-gui-sql-export-final-render-light-800-sql-export-progress-metal.ppm
e0be4e9c6083c955c1dea27c786299c2729db6e5dba760211deca0085f850aa6  /tmp/next-gui-sql-export-final-render-light-800-sql-export-progress.ppm
48e94d7f6b0f67b766c8112e6ec43085dde753c235073579df95e360958c4a4f  /tmp/next-gui-sql-export-native-dark-exportsql/e28_db_browser.png
1c9e0ad5340f68e6f25737f752ed1839e709a83c5e0d8c79f4f69ce4bb48fe65  /tmp/next-gui-sql-export-native-dark-sql-export-canceling/e28_db_browser.png
b7fd8bc83410a80d400a69d98a05bca806b2eb0fbd5eed21f448f64fdc67b68d  /tmp/next-gui-sql-export-native-dark-sql-export-data/e28_db_browser.png
085202697ad35926bca7209c3aae0b23110d083a7ab669361edd7625bbfd0fbc  /tmp/next-gui-sql-export-native-dark-sql-export-progress/e28_db_browser.png
ce3b75805accb50426a5cc994da84c832eb0e25ce3f315834022e1ff3e9bee09  /tmp/next-gui-sql-export-native-light-exportsql/e28_db_browser.png
770e7e3ff154c5cbf6eb20cfe6d0efe0cde78c035f64f5a56b7fed6151cb1a8f  /tmp/next-gui-sql-export-native-light-sql-export-canceling/e28_db_browser.png
e5e2bf671fd3d5d8c091df263c6fb42c380d383df9eb3eb47c11ebe86521bbc8  /tmp/next-gui-sql-export-native-light-sql-export-data/e28_db_browser.png
83459f4e8171a587821ef21ecdde975a26da17130850f93ee10596993c1aabdb  /tmp/next-gui-sql-export-native-light-sql-export-progress/e28_db_browser.png
```
