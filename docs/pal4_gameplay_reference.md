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
match these examples, though their UI behavior is stubbed.
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

The current state work does **not** expose an in-game equipment menu, establish
standalone unequip interactions, preserve forged instance properties, implement
attached-skill effects, or infer consumable healing from skill IDs. Attached-skill
references are loaded as data for later work, not silently treated as completed
gameplay.

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
