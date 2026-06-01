//! Round-trip tests for the agent_server wire protocol.

use agent_server::protocol::{
    AgentCommand, AgentError, AgentErrorKind, AgentResponse, AxisInputParams, DialogSnapshot,
    FastForwardParams, KeyAction, KeyInputParams, LogRecordPayload, LogTailParams, LogTailResponse,
    PartyMember, ScreenshotResponse, ScriptEvalParams, ScriptEvalResponse, SlotParams,
    StateSnapshot, StepTimeParams, TeleportParams,
};

fn roundtrip_command(cmd: &AgentCommand) {
    let json = serde_json::to_string(cmd).expect("serialize");
    let back: AgentCommand = serde_json::from_str(&json).expect("deserialize");
    let json2 = serde_json::to_string(&back).expect("re-serialize");
    assert_eq!(json, json2, "command JSON did not round-trip: {json}");
}

fn roundtrip_response(resp: &AgentResponse) {
    let json = serde_json::to_string(resp).expect("serialize");
    let back: AgentResponse = serde_json::from_str(&json).expect("deserialize");
    let json2 = serde_json::to_string(&back).expect("re-serialize");
    assert_eq!(json, json2, "response JSON did not round-trip: {json}");
}

#[test]
fn every_command_roundtrips() {
    let cases = [
        AgentCommand::GetState,
        AgentCommand::KeyInput(KeyInputParams {
            key: "F".into(),
            action: KeyAction::Tap,
        }),
        AgentCommand::AxisInput(AxisInputParams {
            axis: "LeftStickX".into(),
            value: -0.5,
        }),
        AgentCommand::TeleportPlayer(TeleportParams {
            player: 0,
            pos: [1.0, 2.0, 3.0],
        }),
        AgentCommand::AdvanceDialog,
        AgentCommand::PauseTime,
        AgentCommand::ResumeTime,
        AgentCommand::StepTime(StepTimeParams {
            frames: 30,
            dt: Some(0.016),
        }),
        AgentCommand::StepTime(StepTimeParams {
            frames: 1,
            dt: None,
        }),
        AgentCommand::FastForward(FastForwardParams { on: true }),
        AgentCommand::SaveSlot(SlotParams { slot: 1 }),
        AgentCommand::LoadSlot(SlotParams { slot: 2 }),
        AgentCommand::LogTail(LogTailParams {
            after_seq: 42,
            n: Some(100),
        }),
        AgentCommand::Screenshot,
        AgentCommand::ScriptEval(ScriptEvalParams {
            function: "giAddMoney".into(),
            args: vec![serde_json::json!(100)],
        }),
    ];
    for c in &cases {
        roundtrip_command(c);
    }
}

#[test]
fn every_response_roundtrips() {
    let cases = [
        AgentResponse::Ok,
        AgentResponse::State(StateSnapshot {
            frame: 7,
            scene: "q01".into(),
            block: "q01_01".into(),
            leader: 2,
            leader_pos: [1.0, 2.0, 3.0],
            party: vec![PartyMember {
                slot: 0,
                level: 10,
                hp: 320,
                max_hp: 400,
                mp: 80,
                max_mp: 100,
                in_team: true,
            }],
            money: 1234,
            quest_percentage: 33,
            dialog: DialogSnapshot {
                open: true,
                text: "hi".into(),
                avatar: "left".into(),
            },
            fast_forward: false,
            paused: true,
            current_script_fn: Some("q01_01_main".into()),
            fps: 60.0,
            dt: 0.0167,
        }),
        AgentResponse::Log(LogTailResponse {
            next_seq: 7,
            dropped: true,
            records: vec![LogRecordPayload {
                seq: 6,
                ts: None,
                level: "info".into(),
                target: "shared::openpal4".into(),
                msg: "hi".into(),
            }],
        }),
        AgentResponse::Screenshot(ScreenshotResponse {
            width: 640,
            height: 360,
            encoded: false,
            rgba: Vec::new(),
        }),
        AgentResponse::Script(ScriptEvalResponse {
            function: "giAddMoney".into(),
            result: Some(serde_json::json!(null)),
        }),
        AgentResponse::Error(AgentError {
            kind: AgentErrorKind::Conflict,
            message: "step while running".into(),
        }),
    ];
    for r in &cases {
        roundtrip_response(r);
    }
}

#[test]
fn unknown_response_variant_via_tag_layout_is_rejected() {
    // Ensures we use the externally-tagged layout (tag = type, content
    // = data) — pickleing an unknown tag must fail closed.
    let bad = r#"{"type":"who_knows","data":{}}"#;
    let err = serde_json::from_str::<AgentResponse>(bad).unwrap_err();
    let s = err.to_string();
    assert!(s.contains("unknown variant"), "got: {s}");
}
