# PAL4 gameplay data and UI reference

This reference records clean-room inspection of shipped binary assets and
compiled scripts for the PAL4 gameplay/UI work. It distinguishes stored facts
from runtime behavior that still needs verification. The initial data survey
used assets only; the follow-up native-state trace below inspects the installed
executable's loaded machine code. No third-party reimplementation or internal
development material was used. The decoder and inspector do not change game state.

## Reproducing the inspection

`pal4_data_inspect` depends on the archive/format crates, not the rendering,
audio, or video engine. Pass an archive from a legally installed game:

```powershell
cargo run --release -p pal4_data_inspect -- --archive 'F:\PAL4\gamedata\database.cpk'
cargo run --release -p pal4_data_inspect -- --archive 'F:\PAL4\gamedata\database.cpk' --entry Pal4db.db --table Property
cargo run --release -p pal4_data_inspect -- --archive 'F:\PAL4\gamedata\database.cpk' --entry Pal4db.db --enumeration EquipType
cargo run --release -p pal4_data_inspect -- --archive 'F:\PAL4\gamedata\ui.cpk'
```

Omitting `--entry` lists archive files. A `.db` entry decodes to JSON with
columns in disk order and positional row values. `--table` / `--enumeration`
select a named section; omitting both decodes the whole database.
Other entries print their byte length, MD5, and first 512 bytes.
`--hex-offset <decimal-offset>` inspects another region without decoding.
Do not commit extracted databases, textures, fonts, or generated data dumps.

The inspected archive SHA-256 values were:

| Archive | SHA-256 |
| --- | --- |
| `gamedata\database.cpk` | `b093dc0cf3d82613a9d54759e3235ea4f8332b016fc5ea9a0063478df03dfc2b` |
| `gamedata\ui.cpk` | `a81ae971616af98dd8550100bc9607fd2f4238a097f2ea1db55f24754bee7522` |
| `gamedata\script.cpk` | `d103d9b65f55573098da288f7ac143859efc063c120b2f4f404ddd994c41c85b` |

All three database entries decoded to their exact ends. The extracted copies
of all 37 script modules and 63 layout XML files matched their packed entries
byte-for-byte, checked by MD5. Counts and numeric examples below describe this
corpus, not guarantees about every distribution or mod.

## Database wire format

Implementation: `yaobow\fileformats\src\pal4\database.rs`.
These are not SQLite databases, nor PAL3 fixed-record GDB files.

All numbers are little-endian. Strings are a `u32` byte length followed by
GBK bytes, without an implicit terminator. The file consists of:

1. String `GAME_DB_FLAG`, marker `0x01321844`, enum count.
2. Each enum: name, marker `0x0132188f`, description, value count,
   `(i32 value, string label)` pairs, marker `0x01321890`.
3. Table count, followed by the tables below. There is no final file marker.

Each table contains a name, marker `0x01321841`, stored name, description,
an unidentified header word, column count, and row count. The header word
is zero throughout the inspected corpus and is preserved, not interpreted.
Each column contains a name, type ID, enum-reference string (possibly empty),
and description. Each row is enclosed by `0x0132183f` / `0x01321840`;
`0x01321842` ends the table.

| Type ID | Stored value |
| --- | --- |
| 1 | Signed 32-bit integer, including enum/boolean values |
| 2 | Length-prefixed GBK string |
| 3 | IEEE-754 32-bit float |

Only these types were observed. Unknown types, invalid strings, truncated
records, incorrect markers, and trailing data are errors, not guessed layouts.

| Entry in `database.cpk` | Enums | Tables | Rows |
| --- | ---: | ---: | ---: |
| `Pal4db.db` | 28 | 28 | 4,461 |
| `combatDb.db` | 0 | 1 | 229 |
| `shopdb.db` | 2 | 2 | 899 |

## Gameplay tables and IDs

| `Pal4db.db` table | Rows | Relevant fields |
| --- | ---: | --- |
| `Role` | 4 | `ROLE_ID`, `RA_NAME`, `RA_ICON`, `RA_MODEL`, `CA_*`, `EFE_*` |
| `UpgradeData` | 396 | `ROLE_ID`, `CA_LEVEL`, `CA_EXPERIENCE`, `CA_*`, `EFE_*` |
| `UpgradeParam` | 4 | Named per-character growth parameters |
| `Equip` | 188 | `ID`, `TYPE`, `EA_NAME`, descriptions, model/texture, price, level limits, stat modifiers |
| `EquipRelation` | 7 | `ID`, `CLASS`, `TYPE` |
| `Property` | 246 | `ID`, `PA_NAME`, descriptions, model/texture, category, target, use flags, price, attached skill |
| `Skill` | 700 | `ID`, `TYPE`, `TRIGER`, `FUNCTION`, nine float parameters, attached skill |
| `Prescription` | 150 | Recipe data; decoding does not imply crafting is implemented |
| `GameString` | 315 | UI strings, including save-page labels |

`shopdb.db` contains `ShopProperty` (39 rows) and `Good` (860 rows).
Its `GoodsType` enum distinguishes equipment (0), properties (1), and recipes
(2). Do not merge these namespaces solely on a display name.

### Characters and progression

| Runtime player slot | Database `ROLE_ID` | Character | `RA_MODEL` |
| ---: | ---: | --- | --- |
| 0 | 1001 | Yun Tianhe | `101` |
| 1 | 1002 | Han Lingsha | `103` |
| 2 | 1003 | Liu Mengli | `106` |
| 3 | 1004 | Murong Ziying | `105` |

`UpgradeData` has levels 1 through 99 for each role. Its level-one values
are **not the same as `Role`'s base values**. For role 1001, `Role` stores
`CA_MAX_HP=90`, whereas level-one `UpgradeData` stores `CA_MAX_HP=210`;
both store `CA_MAX_MP=30`. Do not add these together or select one silently.
The initial survey did not establish the native initialization/level-change
policy or experience interpretation. The follow-up native trace below resolves
initialization, explicit level assignment, and experience bounds for the
inspected executable. Combat progression and growth-parameter use are not yet
implemented.

Preserve separate base/progression/equipment contributions in the eventual
model. A stored field name establishes its data meaning, not the game's exact
calculation order or clamping policy.

### Inventory and equipment

Property IDs span 3000 through 3258, with gaps; equipment IDs span 3501
through 3688. Load the records rather than treating those ranges as validation.

`PropertyClass`: recovery 0, attack 1, support 2, material 3, story 4.
The corpus has 33, 32, 36, 112, and 33 records respectively.
`Property` explicitly stores four `ePT_*_CAN_USE` flags,
`CANUSE_IN_SYSUI`, `CANDROP`, `TARGET`, `PA_PRICE`, and `PA_ATTACHED_SKILL`.
Inventory availability is therefore not sufficient to permit use or sale.

`EquipType`: sword 0, dual swords 1, zither 2, head 3, body 4, feet 5,
accessory 6. `EquipRelation` maps types 0/1/2 to class 0, then types
3/4/5/6 to classes 1/2/3/4. The four `ePT_*_LV_LMT` columns hold
per-character level-limit values. Zero means no eligible level in examples
such as equipment 3501, but its full native handling still needs verification.
Equipment 3501 stores limits `[1, 0, 0, 1]`, price 300, and
`CA_PHYSICAL=10`. It is not a generic item usable by every character.

Property 3000 stores recovery category 0, friendly-single target 1,
price 30, system-use flag 1, and attached skill 7200.
Skill 7200 stores type 3 (battle/nonbattle mixed), function 9118,
parameters `[1, 0, 0, 2, 300, 0, 0, 0, 0]`, and attached skill 17062.
This is a verified link, **not yet a verified implementation of healing**:
function IDs, attribute selectors, parameter modes, and chained effects need
independent confirmation before an adventure-item dispatcher is written.
All nonzero property-to-skill references resolved in this corpus.

Every `PA_ICON` and `EA_ICON` field is empty. Matching 3D assets do exist:
property 3000 names model `DHA` / texture `DHA1`, found as
`DHA\DHA.dff` and `DHA\DHA1.png` in `palobject.cpk`; equipment 3501
names `WTA` / `WTA1`, found in `palweapon.cpk`. Do not invent an item-icon
filename scheme. Native preview composition still requires visual confirmation.

## Native UI asset map

The following paths are inside `gamedata\ui.cpk`, mounted by the existing
asset system beneath `gamedata\ui`. A full CEGUI runtime is not currently
implemented: loading a layout paints its static content, not live lists,
formatted editboxes, sliders, role selection, or gameplay actions.

| Layout under `layouts\` | Important named widgets |
| --- | --- |
| `sysToolBar.xml` | `Role`, `Property`, `Equipment`, `Magic`, `Forging`, `Task`, `System` |
| `frameToolbar.xml` | `btnClose` |
| `RoleStateWindow.xml` | Dynamic stat text/editboxes; compose with character selection |
| `RoleSelectorWindow.xml` | Character-selection composition |
| `PropertyWindow.xml` | `PropertyList`, `PropertyDescription`, `PropertyProp`, category buttons |
| `EquipmentWindow.xml` | `List`, `StaticDescription`, `StaticProperty`, `StaticAttr1..10`, `ImageRoleEquip1..6` |
| `ScenarioDialog.xml` | `ScenarioDialog`, `EditDialog`, `ImgClick`, `ImgPortrait` |
| `SelectDialog.xml` | `EditQuestion`, `StaticAnswer` |
| `CommonDialog.xml` | `BtYes`, `BtNo`, `BtOk` |
| `loadWindow.xml`, `savewindow.xml` | `List`, `BtnClose`, `BtnLoad0..9` |
| `SystemSetting.xml` | `BtnSaveData`, `BtnLoadData`, `BtnSystemSetting`, `BtnExit`, `ScroolAudio1`, `ScroolAudio2` |

Property categories use `NewProp`, `PropCure`, `Attack`, `Auxiliary`,
`PropMaterial`, and `PropScenario`. Equipment has `BtnEquipmentClass0..5`
and six equipped-item buttons; `ImageRoleEquip5` and `ImageRoleEquip6`
share the `gy0:tubiao5` image. Preserve six positions rather than inferring
one equipped item per five-value `EquipClass`. Exact slot/order behavior,
including accessory replacement, remains a runtime verification item.

Not every root is a fullscreen canvas: property/equipment content roots
are 800x536; `sysToolBar` is 804x604; dialog/save/load roots are 800x600.
Compose these in a shared fitted coordinate system rather than independently
stretching each root to the viewport.

`ScenarioDialog` uses `zhujiemian0:duihuakuang`, positioned at
`(0, 474)` with size `800x126`. Its `EditDialog` rectangle is
parent-relative `(38, 9, 695, 108)`, with line spacing 3 and text color
`FFFBFFD8`. `ImgClick` uses the `sanjiao` sequence; `ImgPortrait` has
rectangle `(0, 374, 199, 226)`. These are layout defaults, not proof that
the portrait never changes side during play.

Main menu/inventory imagery references sets including `gy0`, `kuang20`,
`jingquedianji0`, `zi0`, and `duihuakuang0`. Resolve the imageset's
declared name and image references; filenames such as `gy_0.imageset`
do not equal the set name.

| Font definition under `fonts\` | Declared name | File | Size |
| --- | --- | --- | ---: |
| `system.font` | `system` | `simsun.ttc` | 13 |
| `SystemBold.font` | `systemBold` | `SystemBold.ttf` | 13 |
| `Dialog_SIMSUN.font` | `dialog_simsun` | `kai.ttf` | 20 |

These are dynamic ideogram fonts with 800x600 native resolution and
autoscaling enabled. Game-owned fonts must remain external assets.
`GameString` IDs 205 through 214 contain the ten page ranges
`1-10` through `91-100`; the four-slot OpenPAL4 menu is an implementation
limit, not the native slot count.

## Compiled-script evidence

Sources are the matching modules from `script.cpk`, parsed with the existing
`ScriptModule` reader and instruction definitions. PCs below are offsets in
the decoded instruction stream, as shown by `csb_inspect --disasm`; they
are not offsets in the packed CPK or compressed CSB file.

For example:

```powershell
cargo run --release -p csb_inspect -- --root F:\PAL4 --file M01 --disasm func2004
cargo run --release -p csb_inspect -- --root F:\PAL4 --file Q01 --disasm func2001
```

Unlike the database inspector, `csb_inspect` currently builds through
`shared` and therefore requires engine/media build prerequisites.

| Module / function / PC | Observation in VM-pop argument order |
| --- | --- |
| `Q01 / func2001 / 0x1aa, 0x1c2` | `giAddPlayerEquip(0, 3586)`, then `(0, 3501)` |
| `M01 / func2004 / 0x5652` | `giPlayerInTeam(1, 0)` |
| `M01 / func2004 / 0x566e, 0x568a` | `giAddPlayerEquip(1, 3526)`, then `(1, 3604)` |
| `M01 / func2004 / 0x56a6` | `giSetPlayerLevel(1, 2)` |
| `M01 / func2004 / 0xa40c` | `giAddProperty(3101, 1, 0)` |
| `M01 / func2004 / 0xa42c` | `giDelProperty(3241, 1, 0)` |
| `Q02 / func5001 / 0x2fc` | `giAddEquipment(3671, 1)` |
| `Q04 / func5003 / 0x2f8` | `giRemoveEquipment(3577, 1)` |
| `Q03 / func5008 / 0x34c` | `giPlayerSetAttr(-1, -1, 100)` |

The third property argument is **not always 1**, contrary to the current
`add_property` implementation comment. Both team functions likewise appear
with second argument 0 and 1. Do not equate that flag to membership itself:
the function name already distinguishes entry/exit. Their flag meanings
(presentation, persistence, formation, or another concern) are unverified.
The seven `giPlayerSetAttr` call sites all use `(-1, -1, 100)`;
that does not establish a general `(player, attribute, value)` signature.

### Runtime menu IDs are not established by the database enum

`Pal4db.db`'s enum named `UI` labels 3 as the item system and 5 as
magic/special skills. However, `M01 / func2004` does this:

- At `0x6128`, `giGrantSystemUi(3, 1)`.
- At `0x613c..0x619c`, disables IDs 0, 1, 2, 5, and 6.
- At `0x61b0`, `giOpenSystemUi(3)`.
- At `0x61bc`, checks `giCheckMagicMastered`.
- On failure, opens ID 3 again at `0x62a8` and loops.
- At `0x638c..0x63fc`, restores IDs 0, 1, 2, 5, and 6.

`Q01 / func2001 / 0x120` separately calls `giGrantSystemUi(4, 0)`.
Thus ID 3 is strongly associated with the magic-learning tutorial,
not evidence that runtime ID 3 should open inventory. Keep the database's
descriptive enum and the runtime menu dispatch mapping separate until verified.
Opening/closing the tutorial menu must preserve the surrounding VM continuation.

### Dialog argument-reading caution

`M10 / func7020 / 0x24a` supplies a string plus two float values
`0.5, 0.5` to `giShowCommonDialog`; `M02 / func1003 / 0x1d8`
supplies one string to `giShowSelectDialog`. Their current argument counts
match these examples; their native presenters and waiting continuations are
implemented in the dialog work below.
`Set4 <local-slot>; GetObjRef 0` replaces a stack placeholder with a
string reference; it does **not** add another argument. Counting those
instructions as two pushes produces a false signature mismatch.

## Boundaries for implementation

The verified inputs above support a data-backed inventory/catalog and explicit
character/equipment models. Before committing native gameplay rules, resolve:

- The authoritative initialization/level-change stats and experience policy.
- Runtime menu IDs, actual unlock persistence, and input/pause behavior.
- `giPlayerSetAttr` sentinels and the ignored property/team flags.
- Equipment transfer/replacement rules, two accessory positions, and stack caps.
- Skill function/parameter semantics for recovery, revival, and conditions.
- Native 3D item previews, font metrics, selection behavior, and save-list rows.

No native-game interaction capture or screenshot comparison was performed in
the initial survey. Static layouts establish controls and geometry, not click behavior,
timing, calculation formulas, or full visual parity. Phase-two state work
must preserve unresolved fields instead of converting hypotheses into rules.

## Follow-up native-state trace

The installed `pal4.exe` has SHA-256
`ed168442c54f1f48332dcb25986c3de85a64c5aef423cdcc61aae2845bb8beee`.
Its on-disk executable is packed. The game was launched normally, and only
its own main executable mapping was read with `ReadProcessMemory` after
initialization. No game memory was modified, no protection was patched, and
the pre-existing analysis database was not consulted. The original save files
were not loaded into the reimplementation or overwritten.

Addresses below are virtual addresses in that executable at image base
`0x00400000`, not portable offsets or assumed signatures for other releases.
AngelScript registration records connect the public script declarations to
their native callbacks; the trace follows those callbacks into state methods.
This establishes implementation behavior for this binary, not a complete
combat model or a visual-parity result.

| Native path | Verified behavior |
| --- | --- |
| `giNewGame` callback `0x005d2180` -> `0x00440a60` / `0x00440b70` -> character reset `0x0043e470` | Four character records are prepared from `Role`; reset then calls the progression setter with level 1. Player zero is enabled as the initial party member. |
| `giSetPlayerLevel` callback `0x005d42d0` -> `0x00580800` -> `0x0057cde0` | Negative player IDs are ignored, not mapped to leader. Valid explicit levels are 1..99. `UpgradeData` is selected by `ROLE_ID=1001+slot` and level, replacing the progression stat block rather than adding `Role` stats. Existing elemental affinities are preserved; equipment contributions are recomputed, then HP/MP are filled. Tianhe therefore starts at 210 HP and 30 MP. |
| Experience helpers `0x004e97f0` / `0x004e96b0` | On explicit level assignment, existing cumulative EXP is clamped between the preceding level's `CA_EXPERIENCE` (zero at level one) and the current level's value minus one. The database value is not the character's initial EXP. |
| `giPlayerSetAttr` callback `0x005d4060` | First argument <0 targets all four characters, including those outside the party. Second argument 0 selects HP, 1 selects MP, -1 selects both; other selectors do nothing. Third argument is percentage of the effective maximum, converted to an integer and clamped by the resource setter. |
| `giSetFullHP` / `giSetFullMP` callbacks `0x005d4610` / `0x005d4660` | Restore every current party member, not only the leader. Non-party characters are unchanged. |
| `giAddPlayerEquip` callback `0x005d4350` -> `0x004c3d20` / `0x0042e5e0` | Creates a new equipment instance and writes its instance ID into a character position. It does not consume a backpack copy or use the menu's level eligibility path. Replaced instances remain owned and become available in the backpack. Negative player IDs are ignored. |
| Position selection `0x0042e540` | Weapon/head/body/feet occupy positions 0..3. An accessory uses position 4 if empty, otherwise 5 if empty, otherwise replaces position 4. Two copies of the same equipment definition can occupy both accessory positions because native state stores instance IDs, not definition IDs. |
| Menu eligibility `0x00450806..0x0045082d`, replacement `0x0044f970` | The character-specific level limit must be positive and no greater than the character's level. Menu equipping writes an existing owned instance into the selected position, leaving the replaced instance owned but unequipped. The foundation exposes a validated backpack-to-equipped transaction separately from script grants. |
| Recalculation `0x0043ecc0` -> `0x0042e630` / `0x004ce900` | Copy base stats, add equipped-instance integer/rate modifiers, then apply active effects and constrain resources. The current foundation implements unmodified database equipment contributions; forging, injected equipment skills, and combat status effects are not implemented. |
| `giAddProperty` / `giDelProperty` callbacks `0x005d3840` / `0x005d3970` | Third argument is a boolean controlling acquisition/removal notification, not persistence. The inventory mutation happens before that branch. Negative amounts reverse add/remove direction. |
| Property storage `0x004c6860` / `0x004c6a30` | A property stack is capped at 99, for both new and existing stacks; removal deletes exhausted stacks. |
| `giAddEquipment` / `giRemoveEquipment` callbacks `0x005d4440` / `0x005d75a0` | Second argument is also a notification boolean. False must not reverse or suppress the mutation. Add creates one instance; remove selects one matching unequipped instance, leaving equipped copies alone. |

`GameplayCatalog` loads metadata and all numeric `CA_*` / `EFE_*` contributions
from the mounted database. The persistent model keeps progression/base and
effective contributions separate, represents six equipped positions, and
keeps backpack counts separate from script-equipped grants. Save JSON includes
these fields; this schema change intentionally has no legacy-save migration.

The phase-two state work did not expose a menu or implement item effects.
The phase-three slice below adds those interfaces for verified recovery forms.
Standalone unequip interactions, forged instance properties, and general
attached-skill/status-effect handling remain unimplemented.

Run the original-data integration check without committing extracted data:

```powershell
$env:PAL4_ASSET_ROOT = 'F:\PAL4'
cargo test --release -p shared --test pal4_gameplay_data -- --ignored
```

A live agent-driven check also reached the idle Q01 opening with Tianhe at
210/30 HP/MP, verified the original opening's equipment grants (3501 and 3586)
in a newly written temporary save, and reloaded that save with identical party
resources. The temporary slot was removed afterwards; no existing slot was
overwritten. This is a state/persistence check, not native UI parity.

## Phase-three gameplay menu

The functional p7 menu is `yaobow\yaobow\scripts\openpal4\game_menu.p7`.
It reads and mutates the authoritative session through `IPal4MenuState`;
there is no separate script-owned inventory or stat model. Escape opens/closes
the menu during free movement. Inside the menu, 1/2/3 select status/items/
equipment, Q/E change party member, arrows select items/change pages, and
Space confirms a supported use/equip action. Mouse controls use press/release
latching in a shared letterboxed 800x600 coordinate system.

Native runtime menu IDs were verified independently of the database `UI` enum:

| Runtime ID | Menu |
| --- | --- |
| 0 | Status |
| 1 | Properties/inventory |
| 2 | Equipment |
| 3 | Magic |
| 4 | Forging |
| 5 | Quests |
| 6 | System |

`giOpenSystemUi` at `0x005d60b0` sends `ID+1` to the toolbar dispatcher
at `0x00482a20`; the toolbar's widget references establish the mapping.
`giGrantSystemUi` at `0x005d6080` changes flag `8+ID`; initialization
`0x005c1b40` enables these flags by default. Unlocks are now saved.
Only the first three tabs are implemented. Unsupported script-open requests
log a warning instead of displaying the wrong menu or suspending indefinitely.
Supported script-open requests yield until the menu closes. Menu dismissal
does not alter a cutscene's persistent control lock.

Recovery is decoded from `Skill`, not hardcoded item IDs. Native function
9118 dispatches to `0x0057fbe0`; parameter groups 4/5/6 and 7/8/9 describe
attribute, amount and mode. Attribute 2 is current HP and 6 is current MP
(registrations at `0x005808d0` onward). Helper `0x005806f0` establishes mode
0 as an additive value and mode 4 as an effective-maximum multiplier.
Function 9132 dispatches to visual-effect callback `0x005803c0`, so the
common attached skill 17062 adds no character-stat mutation.

The supported subset requires an immediate, deterministic, nonbattle-compatible
recovery chain, only HP/MP restoration, and no unknown status/mutex effects.
It honors property menu-use/character flags and friendly-single/party targets,
does not revive, and consumes one item only after a supported state change.
Unsupported chains and no-op/full-resource actions are conservatively disabled;
this is not a claim that every native item-use rejection rule is reproduced.
Visual effects are not played by the menu yet.

The initial functional layout used custom controls over the original
status/property/equipment artwork. The native composition increment below
replaces that opaque overlay.
Item icon fields are empty in the inspected database; per-item icons are not
fabricated. The model-preview increment below renders original 3D assets
instead. Native preview orientation and animation are described below.

Validation includes recorded-host p7 keyboard/mouse interactions, disabled tabs,
cutscene-lock preservation, and original-data single/party recovery tests.
A live Q01 test loaded an isolated temporary save fixture with low HP and owned
items, opened the menu, verified held movement was blocked, consumed property
3000 once, equipped head item 3558, saved and reloaded the results, and verified
the menu closed after reload. The fixture slot was removed; existing slots
were not overwritten. Original chest/reward route validation remains phase six.

### Native portrait entry and menu composition

The exploration HUD now paints the original upper-right `portrait.xml`
`BtnSystem` entry at native `(715,15,68,68)`. Clicking and releasing it opens
the gameplay menu; it is hidden while cutscene/script control prevents opening.
Portrait variants follow the current leader. The HUD uses background image
draws without an invisible fullscreen input-capturing window.

The menu composes `RoleStateWindow.xml` / `PropertyWindow.xml` /
`EquipmentWindow.xml`, `sysToolBar.xml`, `frameToolbar.xml`, and `gameInfo.xml`
on the same fitted 800x600 canvas. Original normal/hover/pressed/disabled art
drives the bottom tabs, right-side categories and close control. Live stats,
resource gauges, quantities, descriptions and equipped names are painted into
their declared native widgets, rather than into a custom panel over the art.
The right-side frame is drawn behind content/category buttons. Unsupported
toolbar tabs remain disabled. The model previews described below now fill the
empty preview region. Native hover sequences and preview rotation are implemented;
decorative particle effects are outside this menu-state/UI slice.

The native close button resolves to `(754,554,40,40)`: its layout root already
contributes `(695,165)`. The shared layout handle now exposes cached absolute
widget bounds and parent IDs, and applies the root offset only once.
Background-only frame widgets (`BackgroundImage` / `olBackgroundImage`) also
produce draw commands. Named image replacement reuses each layout's uploaded
imagesets for leader and party portrait variants.

Keyboard controls remain available. Native inventory rows use release selection
and double-click or Space to perform a supported action; wheel and arrow keys
change list pages, and the header arrows / Q/E change the selected party member.
The first category is the native recent-items view, not an all-items alias.
Rage now reads the saved current resource and drives the native gauge.
Playtime uses the session clocks described below.

Recorded-host regressions cover portrait opening, disabled native tabs,
release-outside cancellation, native close/input isolation and item double-click.
Portrait/close geometry is exercised at 800x600, widescreen and 2x coordinates.
A live asset-backed check captured the
HUD/status/inventory/equipment composition and repeated use/equip/save/reload
through keyboard controls. The initial pointer checks used production p7
hit-testing with injected coordinates. Subsequent dialog validation corrected
the Windows automation to select the actual, nonzero-client-area game window
and use DPI-aware coordinates, enabling physical OS mouse input.

### Completed native menu polish

Property acquisition at `0x004c6860` maintains six recent stack references:
an existing stack is moved to the newest end even when already capped at 99.
Exhausting a stack removes its reference (`0x004c6a30`). The menu shows recent
owned properties newest-first; ordinary categories still include older stock.
Equipment creation (`0x004c3d20`, recent-list handling at `0x004c3e0a`) similarly
retains six acquisition instances, not six distinct definitions. Bounded
acquisition IDs and equipped-slot associations preserve that distinction when
identical old backpack copies coexist with newly equipped copies. Transfers
do not masquerade as new grants. Recency and slot associations are saved.
The existing plain-equipment rows remain grouped by database definition;
forged per-instance modifiers are not introduced here.

Current rage is distinct from `CA_ADDITIONAL_RAGE`. Native initialization
`0x004c80f0` clears the resource array; attribute setter `0x0043fd90`
(rage branch `0x0043fe1c`) constrains index 8 to 0..100. New characters start
at zero, and status text/gauges read this saved resource. Level/equipment
recalculation preserves it within bounds. `giSetFullRage` now restores only
current party members, matching the native loop at `0x005d7ef0`. This is not
a new combat simulation or an invented rage value derived from level data.

`HoverSegImage` animations use the shipped `.seqimage` frames and durations,
sharing the same parser as dialog indicators. The layout handle uploads any
additional frame atlases at construction and samples hover imagery through
`set_animation_time`; rendering never uploads textures mid-frame. Native
`olSelectedImage` is separate from `olPushedImage`, so selected categories and
tabs no longer use a permanently pressed image. Disabled/pressed/hover states
retain their original artwork.

Native property setup `0x00472f70` and equipment setup around `0x0048c8e0`
apply a 30-degree X tilt, initial 10-degree Y orientation, and animated
Y velocity 50 degrees/second. `0x004440a0` stores the enabled rotation vector;
`0x00443f30` integrates it against elapsed time. The existing fitted preview
now reproduces this orientation/spin while retaining manual drag rotation.
Changing selection or reopening resets the preview's initial orientation.

Focused state/recorded-host tests cover acquisition caps, re-grants, exhaustion,
duplicate-equipment ownership, transfer behavior, rage persistence and native
UI text. Original-asset checks resolve all menu hover frames and selected art.
Live Q01 checks use OS mouse input for portrait opening, tabs/categories,
double-click use/equip, preview dragging and closing. They verify real hover
frames, rotating previews, rage/recency save-reload and 1600x900 preview resizing.
The live movement check also caught a missing session binding in staged scene
swaps; that path now binds the same context as synchronous loads, preventing
WASD from leaking through an open menu after loading. Temporary slots are removed
without changing existing saves.

### Native save previews and slot details

Read-only inspection of the original initialized PAL4 executable establishes
that screenshots/details belong inside each **601x91 save row**, not a separate
bottom preview panel. Row construction at `0x004b1ab0` selects `kuang0:kuang21`
for empty rows, `kuang20:kuang21` for populated rows, and
`gy0:xuanzhekuang2` for selection. Painting at `0x004b1f20` places the screenshot
at row-local `(495,7)..(593,82)`: **98x75 pixels**, matching the shipped
`save/0.png`. Title setup (`0x00447099..0x00447100`) and System setup
(`0x0047e16c..0x0047e205`) hide/reparent the `ImgSelectMask`/`StaticSelectMask`
and `ImgDataMask`/`StaticDataMask` templates; they are not fixed preview widgets.
These observations use original local binary/assets only.

The story-detail painter (`0x004b24fa..0x004b2575`) chooses the last
`StoryDesc.ID <= saved MVar`; it does not derive a chapter from quest percentage.
The original save header/serialized `MVar` agree on 10600 for the inspected
slot, selecting the 10500 story entry. The engine maps MVar to shared global 0.
`StoryDesc.CHAPTER`/`MEMO` and `Scene.NAME`, keyed by `SCENE`/`SECTION`, now supply
localized row details alongside saved leader level and elapsed playtime.
Missing metadata falls back to scene/block and progress without inventing a
chapter.

System and title browsers share the native row presenter, selected-row artwork,
leader portrait, details and screenshot. Ten page groups retain the 100-slot
range, with four visible rows and original `gy0` scroll arrows. Keyboard
selection scrolls into view; mouse scrolling does not get undone on idle ticks.
A completed click on a different row selects it without executing a save/load;
activation of the selected row retains the existing action/confirmation flow.
Exact original double-click timing and multi-member portrait composition are
not claimed here.

The thumbnail is an optional PNG inside the atomically published save JSON, so
progress and preview cannot disagree through separately updated sidecars.
Opening a menu freezes the last clean gameplay frame; saving while it remains
open reuses that image rather than capturing the menu. Loading/scene handoff,
movies, script locks and debug-overlay frames cannot supply a new clean preview.
No available frame means no preview, not a failed save. Missing/corrupt/wrong-size
PNG data leaves otherwise valid progress readable. Snapshot-owned sprites keep
texture handles alive across browsing/refresh. The uploader is obtained before
UI painting and queues pixels without borrowing the busy texture cache.
Title artwork and rows use the same draw layer, avoiding native panel images
covering the preview.

Focused tests cover PNG crop/dimensions/limits, metadata predecessor lookup,
corrupt-image fallback, atomic refresh, saved details and keyboard/arrow/wheel
scrolling. Original-asset checks verify localized Q01 and the 10600 story lookup.
Release live checks use app-local agent input and temporary slots 96/100:
title row selection/load, visibly rendered thumbnails, last-slot browsing,
overwrite cancellation, native save/refresh and reload all pass. The saved
thumbnail contains gameplay rather than an open menu. Temporary saves are
removed; existing user slots are not modified.

### Native dialogs and playtime

PAL4's existing Rust VM continuation/presenter boundary now draws the original
`ScenarioDialog.xml`, `CommonDialog.xml` and `SelectDialog.xml` artwork.
No new script bridge is needed for per-character dialog layout. Atlas lookup
reuses the shared internal-name index: names such as `duihuakuang0` and
`kuang20` are not their `.imageset` filenames. Atlas sprites are retained by
the app-lifetime asset loader, including the frame that dismisses a prompt.
Common-message frames use their nine-slice borders rather than stretching
the center image over the whole panel.

All dialog sprites reuse the shared half-texel-inset atlas UV calculation,
including one-texel slice axes and mirrored portraits. The scenario background
is an 800x2 atlas strip stretched to 800x126: sampling its cell boundaries
blended adjacent HUD pixels into colored bands. Whole-texture sampler clamping
does not isolate atlas cells; inset UVs fix both those bands and nine-slice seams.

Scenario text uses the native 20-pixel font size, three-pixel line spacing,
pale default color, portrait position and click indicator. Measured colored
glyphs wrap and page inside the text rectangle, reserving room for an active
left/right portrait. System/selection text uses the shipped `systemBold`
size of 13. Source-layout placeholder text is never shown. Normal colors come
from each text widget's `NormalTextColour`, falling back to CEGUI's inherited
opaque white. In particular, common messages use white, not the selection
question's black. Explicit markup colors are preserved.

The original `OIRAMLOOK.dll` exports establish the rich-text palette, not a
palette inside layout `TextColours`: `XmlColourLabelProcessor::elementStart`
at preferred VA `0x10007ab0` maps `dc0` to the widget default and `dc1`/`dc2`/
`dc3` to opaque red/green/blue. Initializers at `0x100076a0`, `0x100076d0`,
and `0x10007700` establish those RGB values. Native `<br/>` is a real line
break in both rendering and agent text; a break already present after a
speaker colon does not add another blank line. The inherited white default
is initialized by `CEGUIBase.dll`'s `MultiLineEditbox` constructor at preferred
VA `0x1007fc60` (normal color at offset `0x4f8`).

Common messages now wait for acknowledgement; selection/confirmation calls
wait for a real choice. Space/gamepad confirm and directional navigation are
supported, with press/release-latched mouse buttons. Opening/closing input
cannot leak into player movement, and modal input blocking does not freeze
the VM continuation or overwrite the persistent cutscene lock. Agent choices
remain 1-based externally, with the established 0-based select / 1-based
common VM results. Fast-forward skips narration but not plot decisions.
The 0-based default-selection callback is honored (M02 `func1003` explicitly
selects its third, cancel option with value 2). Moving the pointer transfers
the highlight instead of leaving both the old keyboard/default row and the
hovered row highlighted.

Narration reveals progressively. The original game's calls at `0x005bae1b`,
`0x005def36`, and `0x005df228` pass 0.1 seconds to the scenario presenter
`0x004b78c0`. `OIRAMLOOK.dll`'s `OLSDialogEditbox::updateSelf` at preferred
VA `0x100b8ab0` advances once after that interval and clears its timer rather
than batching glyphs after a slow frame. Confirming unfinished text reveals
the current page without dismissing it; a subsequent confirmation advances
or closes it, following the native quick-page mouse behavior at `0x100b8b10`.
The click indicator reads `SegImage`'s actual `.seqimage` file rather than
guessing frames from a name prefix. The shipped `sanjiao.seqimage` specifies
two images, 0.2 seconds each, looping indefinitely.

The original `gameInfo.xml` labels establish `staticTimeInfo` as accumulated
time and `staticGameTimeInfo` as the current session's time. Both now display
HH:MM:SS from active simulation-frame time, including menus and cutscenes.
Agent pause adds no time; stepping adds the requested step duration; fast-forward
does not multiply the clock. Accumulated time is saved/restored with progress.
The current session clock is not rewound by loading another save, and starts
at zero on a new playthrough/session. Old-save migration is not introduced.

Validation covers colored paging, modal input/reset behavior, result/default
indices, clock persistence and recorded-host menu text. Original-asset checks
resolve the shipped dialog layouts and atlas references. Live Q01 captures
verify native narration/portraits; M02 `func1003` verifies the three-option
prompt, Space selecting its declared cancel default, explicit agent choices,
and waiting through fast-forward/invalid choices. A live menu check also
verified that pause adds no time and 60 steps at 0.1 seconds add exactly six
saved seconds. Temporary slots were removed without overwriting existing saves.
Completion checks exercised physical OS mouse hover, release-outside
cancellation and third-choice selection in M02. M10 block `10`'s `tip` trigger
(`func7020`) verified a native common-message frame, readable white text,
physical acknowledgement, and resizing/clicking at 1600x900. Live Q01 captures
verified progressive glyphs, reveal-before-dismiss, both animated click frames,
and subsequent script continuation. Menu/3D-preview pointer and resize checks
remain tracked separately from this completed dialog slice.

### Original item/equipment model previews

The left preview area now renders the selected property's/equipment's actual
`PA_MODEL` / `EA_MODEL` with its database-selected texture. Weapons resolve in
`PALWeapon`; armor/accessories can resolve in `PALObject`. The equipment page
falls back to the currently equipped weapon when its backpack list is empty.
The existing native character portrait is preserved; no extra 3D character
panel is invented.

Preview geometry is read from the original DFF, with material texture names
rewritten on the parsed copy before mesh creation. Namespaced texture-store
keys include model path and selected skin, so shared meshes such as DHA
(properties 3000/3004/3006) and WTA (equipment 3501/3503) cannot retain another
item's pixels. Untextured `hook_*` attachment-marker atomics are excluded from
the preview without deleting their frames or changing the gameplay loader.
Original archive/model files are never modified.

The preview uses a private scene and the existing engine offscreen-render
target path, with an opt-in transparent clear color (editor targets retain
their opaque default). Camera framing fits transformed render-object bounds
and target aspect, rather than assuming a model is centered at the origin.
Framing remains fitted to model bounds rather than copying the original fixed
preview world's geometry; orientation and angular velocity match the native
setup documented above.

The image is clipped between the native left class strip, list panel and
toolbar/portrait boundaries. Drag inside it to rotate. Resolutions are bounded
to 1024 pixels on the longest side while preserving aspect. Model/target work
runs during director update; script-side rendering only queues the request and
samples a ready texture. Missing models log once per selection rather than
retrying every frame or displaying a fabricated substitute.

UI painting precedes director update in `CoreRadianceEngine`. A tab/key change
can therefore retire a target *after* that frame's imgui draw list referenced
it. Retired previews retain their scene, target and descriptor for three
frames before destruction; the texture cache's delayed com-ID eviction alone
does not retain an external target's backend descriptor.

Validation covers original model/skin lookup, camera/extent calculations,
preview-enabled p7 behavior and a live temporary-save fixture. Live checks
switch DHA/WTA texture variants, preview armor, repeatedly
close/reopen, equip an item, restore a save, and exercise the equipped-weapon
fallback. Transparent composition and original model imagery are captured in
session screenshots, not committed as game assets.
The original resize attempt targeted a helper window. Follow-up validation
selects the nonzero-client game HWND and confirms live model-preview resizing
to 1600x900, including save/reload and drag rotation without descriptor errors.

### Native System menu and save/load

The System toolbar entry uses the shipped `SystemSetting.xml` and
`CommonDialog.xml` artwork. `SaveWindow0` and `BtnData0..9` define 100 slots,
ten per page; the title load screen now pages the same range. Rows distinguish
empty, readable, and corrupt/unreadable saves and show scene/block, leader
level, and accumulated playtime. Debug hotkeys still address slots 1–4.
Save previews and battle-specific settings are not fabricated.

Overwrite, load, return-to-title, and quit require an explicit native checkmark
click. Escape cancels the top modal without closing the menu; Space/Enter
does not accept a destructive default. Pending operations and modal/slider
capture block toolbar/close click-through. Music and sound sliders update
separate mixer channels (sound includes voice and spatial ambience), persist
on release, and retain the existing global master gain. Escape during a drag
restores its original gain. The shared mixer preserves PAL3's existing API.

System actions are queued and drained from app pre-update, outside UI/VM
borrows, including launches without an agent server. Saves serialize and sync
a sibling temporary file before publishing; errors remain visible and do not
silently replace the old slot. In-game loads stage the replacement scene under
the loading overlay before applying saved state/globals. The outgoing scene
stays loaded until commit: `pop_scene()` unloads and clears its entities, so
pushing it back after a failed stage is not a valid rollback.

Focused tests cover paging, slot classification, confirmations, input capture,
volume isolation/persistence errors, single-flight gating, and save publication
failure. Live original-asset checks use isolated saves/configuration to exercise
creation, overwrite cancel/accept, successful position restoration, failed
scene loading with the original world still rendered, unwritable destinations,
audio persistence, and return-to-title. Existing user saves are not overwritten.

### Verified adventure-item effects

The next native trace used only the owned original game's executable mapping,
with the same read-only inspection policy and image base described above.
Recovery callback `0x0057fbe0` routes proportional HP/MP through
`0x005806f0`. Its mode-4 paths at `0x00580798` and `0x005807c2` store the
maximum-times-rate result as a 32-bit float before resource application.
Recovery now follows that float arithmetic instead of truncating an extended
precision product too early: 45% of 200 HP restores 90, not 89. Food still
requires a living eligible party member, applies both resource groups, clamps
at effective maxima, and consumes once only when a resource changes.

Revival properties 3029–3031 and condition-removal properties 3032–3033,
3035–3040 and 3049–3055 have `CANUSE_IN_SYSUI=0` in this corpus. They are
not enabled as adventure cures; adding combat conditions or revival semantics
to make them usable here would invent behavior. Their attached chains remain
unsupported by the adventure recovery decoder.

Portable-save property 3182 (元祖星空) is the remaining menu-enabled support
item: category 2, target 0, skill 7294, function 9120. The function registration
at `0x005893a5` points to the no-op `0x004ee840`: this is **not** an ordinary
stat skill. Inventory use at `0x004728a3` explicitly recognizes item 3182 and
calls `0x00473740`. That handler removes one copy through `0x004c6a30` before
dispatching `show_save_ui` (descriptor `0x008a66a4`). Cancellation does not
refund it. The bridge now opens the existing native System save page and
spends one copy, with live availability, story locks and menu grants checked
before either change. No file is written merely by opening the page.
Unknown or modified attached effects are not silently ignored.

Earth-pearl property 3067 is a separate native special case despite its
zero menu-use flag. Its `0x004735c0` handler starts an `EarthBall` scripted
continuation rather than function 9121, which is also registered to the no-op.
Return-destination and continuation handling remain unverified/unimplemented;
the item stays disabled rather than teleporting to a guessed entrance.

Validation includes recorded-host item use, save-page entry/cancellation,
one-copy consumption and rejected duplicate use, story/menu gating, food
rounding, and original-database recovery/adventure integration tests.
Live Q01 checks with isolated slots verified portable-save entry and
cancellation, 45%-HP/15%-MP food restoring 90 HP/4 MP from the controlled
200-HP fixture, refusal to consume a battle-only revival item, and persistence
of recovered resources and spent quantities through save/reload. Existing user
saves were untouched; owned test slots/configuration and the executable
inspection copy were removed.

### Metadata pickups, chest settlement and reward notices

The original GOB survey finds 484 tag-7 `GET_ITEM` objects, all with empty
research callbacks. Ordinary pickups therefore dispatch from their PropID,
EquipID and num metadata, not a fabricated script function. Native handler
`0x0059da60` prefers a positive property ID over equipment, adds the requested
property count (capped at 99), and creates one equipment instance. Its notice
still displays the authored count. Invalid definitions and full stacks reach
the same research-count settlement at `0x00599af0`; overflow is not a retry.
The fixed `object-store` property, not the guessed second GOB flag, controls
persistence. Positive research counts decrement, while negative counts remain
unlimited; auto-disappearing exhausted objects are hidden and disabled.

The survey also finds 133 metadata reward machines (OM07--OM10 chests and OQ20
money pickups). Their task sequences contain animation (3), property (7),
equipment (8) and money (9) tasks, with no item prerequisites. The implementation
handles these nonrepeating, manually activated reward-only sequences without
enabling unrelated mechanisms. Native `0x005a1400` consumes research count when
starting; `0x00594930` grants each reward and `0x00594b80` waits for the hint to
close before proceeding. Opening animation holds its last pose. A stored
scene/gameplay-block/object cursor is saved alongside inventory, so loading
during opening resumes the remaining tasks rather than awarding earlier ones
again. Completed opening poses restore without replaying reward sounds.
Store-zero objects reset on scene reconstruction.

Native animation-task start `0x00592510` plays `WC003.wav` for objects whose
fixed `object-marker` is 4. The sound starts with the opening task, not a guessed
AMF event; the bundled chest ANMs have no matching event files. Resuming an
interrupted opening replays that task's sound, while restoring a completed pose
does not.

The singleton hint at `0x004bf4a0` replaces the previous text and resets its
one-second timer (`0x004ab840`), rather than stacking a guessed toast queue.
The presenter reuses `hintWindow.xml`, its original nine-slice atlas cells,
13-pixel text, pale-yellow default and inline orange item-name markup, centered
at normalized `(0.5, 0.15)`. Notices are transient and cannot contaminate a
clean save thumbnail. Script notification booleans now control feedback without
preventing inventory mutation; signed counts retain each callback's native
acquisition/removal template and display requested rather than accepted amounts.
GameString IDs 251/252 cover properties, 251/126 equipment, and 194/267 money.
WA06/WA07 are acquisition/loss sounds. Native AddMoney plays WA40 even when its
notice flag is false; PayMoney has no reward sound. Money follows the clamp
at `0x004c5870`: zero through 9,999,999.

Native hint sizing at `0x00549b30` measures the text and retains the original
frame-to-text size difference. The text receives 7 extra pixels horizontally
and 1 vertically; combined with the layout's 32/28 frame padding, the result is
measured text width plus 39 and height plus 29, matching the presenter. Exact
native `systemBold` sizing is now reproduced for reward notices.

`SystemBold.font` selects the shipped `SystemBold.ttf` at size 13; it is
FZXiaoBiaoSong-B05, not the Kai dialogue face or the bundled editor font.
CEGUIBase `0x100b6f10` calls the pixel-size routine at `0x10104510`, and glyph
advances are integer pixels (`0x100b4ac1`). The font's em is 256 units with
ascender/descender 231/-74. A 13-pixel em therefore corresponds to imgui's
15.48828125-pixel vertical span, not a normalized 13-pixel imgui font. The
native baseline is 11.73046875 design pixels; the presenter preserves its
fractional offset instead of adding the regular game-font offset or weight.
The dedicated face is loaded through the VFS, including packed installations,
registered between frames, and used for both measurement and drawing. Atlas
glyph advances are rounded at design resolution before canvas scaling.

The original-font regression matches 52-pixel "宝箱已锁", 78-pixel
"矿石不可挖取", and 33-pixel `A0 x3` widths, including 1.6x/2x scaling,
and the first glyph's native 0.73046875-pixel top offset. This verifies
metrics, not pixel-identical antialiasing: the renderer still uses stb instead
of the original FreeType rasterizer. Other dialogue/menu fonts are unchanged.
Live M13 `item201` wrong-leader interaction renders the native bold rejection
notice through the packed font resource, leaves inventory unchanged, and expires
normally. The original-font regression, 58 PAL4 tests, release build, affected
all-target checks and workspace formatting pass. Owned slot 96 and the test
process are removed.

Actor-specific pickups now use the native dispatcher gate at `0x00425950`.
`validfor=1` requires Han Lingsha and rejects with GameString 265 ("宝箱已锁");
`validfor=3` requires Murong Ziying and rejects with 266 ("矿石不可挖取").
Both failures display the singleton hint and play WA05 without claiming the
object or advancing its tasks. Other values do not impose a leader gate.
Native leader assignment at `0x004238b0` maps Player-TIANHE/LINSHA/MENGLI/ZIYING
to 0/1/2/3. Physical and named agent interaction share the eligibility check;
the latter reports 409 for a mismatched leader.

Physical XZ selection accepts the authored radius boundary, as native
`0x00599990` does. For height selection, `0x004253cf` calls nearest
object dispatcher `0x005a6610` with the leader position and tolerance returned
by `0x00443790` (twice the player object's float at offset 0xdc). Height-limited
objects require strict bounds around the player's Y; height-limit zero bypasses
that check. Native `0x00666cf0`/`0x00666f10` derive that float by merging the
actor DFF's atomic bounding spheres in clump-local coordinates, cancelling the
root world transform. DFF loading at `0x006b6ed7` prepends atomics; traversal at
`0x006b6020` therefore reverses serialized order. This matters because sphere
merging is order-dependent. The actor loader now computes and caches the same
diameter during its existing parse, rather than guessing a height constant.
Invalid sphere data is ignored and coincident centers avoid division by zero.

Validation: 58 PAL4 library tests pass with original assets enabled, including
metadata parsing, signed/silent/capped grants, equipment instance counts,
physical selection and transient feedback reset. The release build, affected
all-target checks and workspace format check pass. Live Q01 checks exercise
F-key pickup, repeat rejection, native hint rendering and reward sound lookup,
complete chest grants, settled save/load, and saves at task cursor 0 and 3.
Loading cursor 3 grants only the remaining three portable-save items, without
duplicating the two equipment instances. Test slots 96--98 are removed;
existing slots 1--5 are not overwritten.

The leader-gate follow-up passes nine focused reward tests, including traversal
of the mounted original corpus (104 actor-specific pickup entries), rejection
without settlement, valid leader grants and inclusive XZ-boundary selection.
A live M13/1 `item201` check rejects Tianhe through both agent and F-key input,
allows Ziying to collect through F, rejects repeats, and preserves the claim
after save/load. Temporary slots 96--97 are removed. Release build, affected
all-target checks and workspace formatting pass.

The height/opening follow-up adds synthetic sphere tests for nested rotated
frames, cancelled root translation/scale, contained and coincident spheres,
invalid radii, and reverse traversal. Physical tests reject exact upper/lower
Y boundaries and accept the height-limit-zero bypass. Live Q01 `item008`
rejects F-key pickup at Y offsets +/-79 and accepts +78 using Tianhe's computed
78.49662 diameter; failed attempts leave no claim. The collected claim survives
save/load without duplication. Q01 `item101` still settles its complete chest
rewards, resumes an opening save at cursor 0 with identical inventory, and
rejects repeats after settled save/load. Temporary slots 96--97 and owned test
processes are removed; existing slots 1--5 remain untouched.
