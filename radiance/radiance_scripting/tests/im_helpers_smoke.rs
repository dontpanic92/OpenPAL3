//! Coverage for `radiance_scripting.im` — the immediate-mode table helpers.
//!
//! imgui tables do not auto-advance: the caller must invoke
//! `table_next_column()` before *every* cell. With no helper for that, the
//! editor's welcome grid and settings table were written as manually unrolled
//! sequences (fifteen and ten call pairs respectively) even though a `for`
//! loop was available. `grid` / `grid_of` make the loop form the natural one,
//! so their cell ordering and column advancing are pinned here.

use radiance_scripting::ScriptHost;
use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};

const IM_MODULE: &str = include_str!("../scripts/im.p7");

fn run(screen: &str, ui_com: crosscom::ComRc<radiance::comdef::IUiHost>) {
    let host = ScriptHost::new();
    host.add_binding("radiance_scripting.im", IM_MODULE);
    host.load_source(screen).expect("screen compiles");
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance.comdef.IUiHost", com_id)
        .expect("ui foreign box");
    host.call_returning_data("entry", vec![ui_box])
        .expect("entry runs");
}

/// The recorded call stream reduced to the interesting events: a column
/// advance, a cell (identified by its button label), or a blank.
#[derive(Debug, PartialEq)]
enum Ev {
    NextColumn,
    Cell(String),
    Blank,
}

fn events(calls: &[UiCall]) -> Vec<Ev> {
    calls
        .iter()
        .filter_map(|c| match c {
            UiCall::TableNextColumn => Some(Ev::NextColumn),
            UiCall::Button { label, .. } => Some(Ev::Cell(label.clone())),
            UiCall::Dummy { .. } => Some(Ev::Blank),
            _ => None,
        })
        .collect()
}

#[test]
fn grid_advances_a_column_before_every_cell() {
    let (recorder, ui_com) = RecordingUiHost::create();
    run(
        r#"
import radiance;
import radiance_scripting.im;

pub fn entry(host: box<radiance.IUiHost>) -> int {
    im.grid(host, "t", 2, 3, (i: int) => {
        host.button(f"cell{i}", 10.0, 10.0);
    });
    0
}
"#,
        ui_com,
    );

    let calls = recorder.calls.borrow();
    assert_eq!(
        events(&calls),
        vec![
            Ev::NextColumn,
            Ev::Cell("cell0".into()),
            Ev::NextColumn,
            Ev::Cell("cell1".into()),
            Ev::NextColumn,
            Ev::Cell("cell2".into()),
        ]
    );
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Table { id, cols, .. } if id == "t" && *cols == 2)),
        "the helper must open the table itself; got {calls:?}"
    );
}

/// The editor's game picker places specific games in specific cells with
/// deliberate gaps, so the cells are passed as data rather than a dense range.
/// `grid_of` must preserve that exact ordering and render `blank` ordinals as
/// empty cells — this is what replaced the fifteen hand-unrolled call pairs.
#[test]
fn grid_of_preserves_ordinal_order_and_renders_blanks() {
    let (recorder, ui_com) = RecordingUiHost::create();
    run(
        r#"
import radiance;
import radiance_scripting.im;

let BLANK: int = -1;

pub fn entry(host: box<radiance.IUiHost>) -> int {
    let cells: array<int> = [0, 8, BLANK, 5];
    im.grid_of(host, "games", 3, cells, BLANK, (ord: int) => {
        host.button(f"g{ord}", 10.0, 10.0);
    });
    0
}
"#,
        ui_com,
    );

    let calls = recorder.calls.borrow();
    assert_eq!(
        events(&calls),
        vec![
            Ev::NextColumn,
            Ev::Cell("g0".into()),
            Ev::NextColumn,
            Ev::Cell("g8".into()),
            Ev::NextColumn,
            Ev::Blank,
            Ev::NextColumn,
            Ev::Cell("g5".into()),
        ],
        "ordinals must be emitted in the authored order, with blanks in place"
    );
}

#[test]
fn grid_with_zero_count_emits_an_empty_table() {
    let (recorder, ui_com) = RecordingUiHost::create();
    run(
        r#"
import radiance;
import radiance_scripting.im;

pub fn entry(host: box<radiance.IUiHost>) -> int {
    im.grid(host, "t", 2, 0, (i: int) => {
        host.button(f"cell{i}", 10.0, 10.0);
    });
    0
}
"#,
        ui_com,
    );

    let calls = recorder.calls.borrow();
    assert_eq!(events(&calls), vec![]);
    assert!(calls.iter().any(|c| matches!(c, UiCall::Table { .. })));
}
