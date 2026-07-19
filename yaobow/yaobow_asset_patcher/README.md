# Yaobow PAL3 Mod Manager

`yaobow_asset_patcher` imports, installs, and transactionally uninstalls
`.ybpatch` mods. Imported patches and manager state are stored under the
selected PAL3 installation's `.yaobow_patch/` directory.

The GUI uses p7-lcl v0.1.0. Release archives contain this layout:

```text
yaobow_asset_patcher[.exe]
p7-lcl/
  LICENSE
  THIRD_PARTY_NOTICES.md
  src/mod.p7
  native/lib/<platform library>
```

For a development build, extract the matching p7-lcl release and set
`YAOBOW_P7_LCL_DIR` to its root before launching the executable.

Ubuntu 24.04 requires the `libgtk-3-0t64` and `libharfbuzz-gobject0` runtime
packages. Other supported targets use the native Win32 or Cocoa widget set.

To stage the pinned release resources:

```bash
python3 scripts/stage-p7-lcl.py \
  --target aarch64-apple-darwin \
  --output target/asset-patcher-package
```
