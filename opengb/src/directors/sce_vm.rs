use super::{global_state::GlobalState, sce_commands::*};
use crate::{asset_manager::AssetManager, loaders::sce_loader::SceFile};
use encoding::{DecoderTrap, Encoding};
use imgui::*;
use log::{debug, error, warn};
use radiance::input::InputEngine;
use radiance::media::MediaEngine;
use radiance::scene::{Director, SceneManager};
use std::fmt::Debug;
use std::{
    any::Any,
    cell::{Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};

pub struct SceExecutionOptions {
    pub proc_hooks: Vec<Box<dyn SceProcHooks>>,
}

pub trait SceProcHooks {
    fn proc_begin(&self, sce_name: &str, proc_id: u32, global_state: &mut GlobalState);
    fn proc_end(&self, sce_name: &str, proc_id: u32, global_state: &mut GlobalState);
}

pub struct SceVm {
    state: SceState,
    active_commands: Vec<Box<dyn SceCommand>>,

    debug_proc: String,
    debug_scn_name: String,
    debug_scn_subname: String,
    debug_main_story: String,
}

impl SceVm {
    pub fn new(
        media_engine: Rc<dyn MediaEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        sce: SceFile,
        sce_name: String,
        asset_mgr: Rc<AssetManager>,
        global_state: GlobalState,
        options: Option<SceExecutionOptions>,
    ) -> Self {
        let state = SceState::new(
            input_engine.clone(),
            media_engine.clone(),
            asset_mgr.clone(),
            Rc::new(sce),
            sce_name,
            global_state,
            options,
        );

        Self {
            state,
            active_commands: vec![],
            debug_proc: String::from(""),
            debug_scn_name: String::from(""),
            debug_scn_subname: String::from(""),
            debug_main_story: String::from(""),
        }
    }

    pub fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        self.state.global_state_mut().update(delta_sec);

        if self.active_commands.len() == 0 {
            loop {
                match self.state.get_next_cmd() {
                    Some(mut cmd) => {
                        cmd.initialize(scene_manager, &mut self.state);
                        if !cmd.update(scene_manager, ui, &mut self.state, delta_sec) {
                            self.active_commands.push(cmd);
                        }
                    }
                    None => {
                        self.state.global_state_mut().set_adv_input_enabled(true);
                        return None;
                    }
                };

                if self.state.run_mode() == 1 && !self.active_commands.is_empty() {
                    break;
                }
            }
        } else {
            let state = &mut self.state;
            self.active_commands
                .drain_filter(|cmd| cmd.update(scene_manager, ui, state, delta_sec));
        }

        None
    }

    pub fn render_debug(&mut self, scene_manager: &mut dyn SceneManager, ui: &Ui) {
        ui.text(format!("Active commands: {}", self.active_commands.len()));

        imgui::InputText::new(ui, "Sce Proc Id", &mut self.debug_proc).build();
        if ui.button("Execute") {
            println!("{}", self.debug_proc);
            if let Ok(id) = self.debug_proc.parse::<u32>() {
                self.call_proc(id);
            }
        }

        imgui::InputText::new(ui, "Target Scn Name", &mut self.debug_scn_name).build();
        imgui::InputText::new(ui, "Target Scn SubName", &mut self.debug_scn_subname).build();
        if ui.button("Load") {
            self.active_commands.push(Box::new(SceCommandLoadScene::new(
                self.debug_scn_name.to_string(),
                self.debug_scn_subname.to_string(),
            )));
        }

        imgui::InputText::new(ui, "Main Story", &mut self.debug_main_story).build();
        if ui.button("Set") {
            if let Ok(value) = self.debug_main_story.parse::<i32>() {
                self.active_commands
                    .push(Box::new(SceCommandLet::new(-32768, value)));
            }
        }

        let commands = self
            .active_commands
            .iter()
            .fold("".to_string(), |acc, next| {
                let debug: &dyn SceCommandDebug = next.as_ref();
                format!("{}\n{}", acc, debug.debug())
            });
        ui.text(format!("{}", commands));
    }

    pub fn call_proc(&mut self, proc_id: u32) {
        self.state.call_proc(proc_id)
    }

    pub fn state(&self) -> &SceState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut SceState {
        &mut self.state
    }

    pub fn global_state(&self) -> &GlobalState {
        &self.state.global_state
    }

    pub fn global_state_mut(&mut self) -> &mut GlobalState {
        &mut self.state.global_state
    }
}

macro_rules! command {
    ($self: ident, $cmd_name: ident $(, $param_names: ident : $param_types: ident)* $(,)*) => {
        command! {@reverse_arg $self, $cmd_name $(, $param_names : $param_types)* [] [$(, $param_names)*]}
    };

    (@reverse_arg $self: ident, $cmd_name: ident, $param_name: ident : $param_type: ident $(, $param_names: ident : $param_types: ident)* [$(, $reversed_names: ident : $reversed_types: ident)*] [$(, $asc_param_names :ident)*]) => {
        command! {@reverse_arg $self, $cmd_name $(, $param_names : $param_types)* [, $param_name : $param_type $(, $reversed_names : $reversed_types)*] [$(, $asc_param_names)*]}
    };

    (@reverse_arg $self: ident, $cmd_name: ident [$(, $reversed_names: ident : $reversed_types: ident)*] [$(, $asc_param_names :ident)*]) => {
        command! {@inner $self, $cmd_name $(, $reversed_names : $reversed_types)* [] [$(, $asc_param_names)*]}
    };

    (@inner $self: ident, $cmd_name: ident $(, $param_names: ident : $param_types: ident)* [$(, $evaluated: ident)*] [$(, $asc_param_names :ident)*]) => {
        {
            $(let $param_names = data_read::$param_types($self);)*
            debug!(concat!("{} ", $(concat!("{", stringify!($asc_param_names), ":?} "), )*), stringify!($cmd_name), $($asc_param_names=$asc_param_names, )*);
            Some(Box::new($cmd_name::new($($asc_param_names),*)))
        }
    };
}

macro_rules! nop_command {
    ($self: ident, $cmd_name: ident $(, $param_types: ident)* $(,)*) => {
        nop_command! {@reverse_arg $self, $cmd_name $(, $param_types)* []}
    };

    (@reverse_arg $self: ident, $cmd_name: ident, $param_type: ident $(, $param_types: ident)* [$(, $reversed_types: ident)*]) => {
        nop_command! {@reverse_arg $self, $cmd_name $(, $param_types)* [, $param_type $(, $reversed_types)*]}
    };

    (@reverse_arg $self: ident, $cmd_name: ident [$(, $reversed_types: ident)*]) => {
        nop_command! {@inner $self, $cmd_name $(, $reversed_types)*}
    };

    (@inner $self: ident, $cmd_name: ident $(, $param_types: ident)*) => {
        {
            $(let _ = data_read::$param_types($self); )*
            warn!("Unimplemented command: {}", stringify!($cmd_name));
            Some(Box::new(SceCommandNop::new()))
        }
    };
}

pub struct SceProcContext {
    sce: Rc<SceFile>,
    proc_id: u32,
    program_counter: usize,
    local_vars: HashMap<i16, i32>,
    dlgsel: i32,
}

impl SceProcContext {
    pub fn new_from_id(sce: Rc<SceFile>, proc_id: u32) -> Self {
        let index = sce
            .proc_headers
            .iter()
            .position(|h| h.id == proc_id)
            .unwrap();
        Self::new(sce, index)
    }

    pub fn new_from_name(sce: Rc<SceFile>, proc_name: &str) -> Option<Self> {
        sce.proc_headers
            .iter()
            .position(|h| h.name.to_lowercase() == proc_name.to_lowercase())
            .and_then(|index| Some(Self::new(sce, index)))
    }

    fn new(sce: Rc<SceFile>, index: usize) -> Self {
        let proc = &sce.proc_headers[index];
        let proc_id = proc.id;

        debug!(
            "Start executing SceProc {} Id {} Offset {}",
            proc.name, proc.id, proc.offset
        );
        Self {
            sce,
            proc_id,
            program_counter: 0,
            local_vars: HashMap::new(),
            dlgsel: 0,
        }
    }

    pub fn set_local(&mut self, var: i16, value: i32) {
        self.local_vars.insert(var, value);
    }

    pub fn get_local(&mut self, var: i16) -> Option<i32> {
        self.local_vars.get(&var).and_then(|v| Some(*v))
    }

    pub fn set_dlgsel(&mut self, value: i32) {
        self.dlgsel = value;
    }

    pub fn get_dlgsel(&self) -> i32 {
        self.dlgsel
    }

    fn get_next_cmd(&mut self) -> Option<Box<dyn SceCommand>> {
        if self.proc_completed() {
            return None;
        }

        let cmd = data_read::i16(self);
        let access_local_var = data_read::i16(self);
        match cmd {
            1 => {
                // Idle
                command!(self, SceCommandIdle, length: f32)
            }
            2 => {
                // ScriptRunMode
                command!(self, SceCommandScriptRunMode, mode: i32)
            }
            3 => {
                // Goto
                command!(self, SceCommandGoto, offset: u32)
            }
            5 => {
                // FOP
                command!(self, SceCommandFop, op: i32)
            }
            6 => {
                // GT
                command!(self, SceCommandGt, var: i16, value: i32)
            }
            7 => {
                // LS
                command!(self, SceCommandLs, var: i16, value: i32)
            }
            8 => {
                // EQ
                command!(self, SceCommandEq, var: i16, value: i32)
            }
            9 => {
                // NEQ
                command!(self, SceCommandNeq, var: i16, value: i32)
            }
            10 => {
                // GEQ
                match access_local_var {
                    1 => command!(self, SceCommandGeq, var: i16, value: i32),
                    3 => command!(self, SceCommandGeq2, var: i16, var2: i16),
                    _ => nop_command!(self, GeqNotSupported),
                }
            }
            11 => {
                // LEQ
                command!(self, SceCommandLeq, var: i16, value: i32)
            }
            12 => {
                // TestGoto
                command!(self, SceCommandTestGoto, offset: u32)
            }
            13 => {
                // Let
                command!(self, SceCommandLet, var: i16, value: i32)
            }
            16 => {
                //Call
                command!(self, SceCommandCall, proc_id: u32)
            }
            17 => {
                // Rnd
                command!(self, SceCommandRnd, var: i16, value: i32)
            }
            19 => {
                // Between
                command!(self, SceCommandBetween, var: i16, lb: i32, rh: i32)
            }
            20 => {
                // RolePathTo
                command!(
                    self,
                    SceCommandRolePathTo,
                    role_id: i32,
                    x: i32,
                    y: i32,
                    unknown: i32
                )
            }
            21 => {
                // RoleSetPos
                command!(self, SceCommandRoleSetPos, role_id: i32, x: i32, y: i32)
            }
            22 => {
                // RoleShowAction
                command!(
                    self,
                    SceCommandRoleShowAction,
                    role_id: i32,
                    action_name: string,
                    repeat_mode: i32,
                )
            }
            23 => {
                // RoleSetFace
                command!(self, SceCommandRoleSetFace, role_id: i32, direction: i32)
            }
            24 => {
                // RoleTurnFace
                command!(self, SceCommandRoleTurnFace, role_id: i32, degree: f32)
            }
            25 => {
                // TeamOpen
                nop_command!(self, TeamOpen)
            }
            26 => {
                // TeamClose
                nop_command!(self, TeamClose)
            }
            27 => {
                // RoleInput
                command!(self, SceCommandRoleInput, enable_input: i32)
            }
            28 => {
                // RoleActive
                command!(self, SceCommandRoleActive, role: i32, active: i32)
            }
            29 => {
                // RoleScript
                command!(self, SceCommandRoleScript, role: i32, proc_id: i32)
            }
            30 => {
                // CameraFocusRole
                nop_command!(self, CameraFocusRole, i32)
            }
            31 => {
                // CameraFocusPoint
                nop_command!(self, CameraFocusPoint, f32, f32, f32)
            }
            32 => {
                // CameraPush
                nop_command!(self, CameraPush, f32, f32, i32)
            }
            33 => {
                // CameraRotate
                nop_command!(self, CameraRotate, f32, f32, f32, i32)
            }
            34 => {
                // CameraMove
                command!(
                    self,
                    SceCommandCameraMove,
                    position_x: f32,
                    position_y: f32,
                    position_z: f32,
                    unknown_1: f32,
                    unknown_2: f32
                )
            }
            35 => {
                //CameraWag
                nop_command!(self, CameraWag, f32, f32, f32, i32)
            }
            36 => {
                // CameraSet
                command!(
                    self,
                    SceCommandCameraSet,
                    y_rot: f32,
                    x_rot: f32,
                    unknown: f32,
                    x: f32,
                    y: f32,
                    z: f32,
                )
            }
            37 => {
                // CameraDefault
                command!(self, SceCommandCameraDefault, unknown: i32)
            }
            38 => {
                // CameraPushState
                nop_command!(self, CameraPushState)
            }
            39 => {
                // CameraPopState
                nop_command!(self, CameraPopState)
            }
            42 => {
                // LK_Ghost
                nop_command!(self, LK_Ghost, i32)
            }
            43 => {
                // FavorAdd
                nop_command!(self, FavorAdd, i32, i32)
            }
            46 => {
                // AddItem
                nop_command!(self, AddItem, i32, i32)
            }
            47 => {
                // RemoveItem
                nop_command!(self, RemoveItem, i32)
            }
            48 => {
                // AddMoney
                nop_command!(self, AddMoney, i32)
            }
            49 => {
                // GetMoney
                command!(self, SceCommandGetMoney, var: i16)
            }
            50 => {
                // GetFavor
                nop_command!(self, GetFavor, i16, i32)
            }
            51 => {
                // AddSkill
                nop_command!(self, AddSkill, i32, i32)
            }
            52 => {
                // GetFavorite
                nop_command!(self, GetFavorite, i16)
            }
            54 => {
                // FullRoleAtt
                nop_command!(self, FullRoleAtt, i32, i32)
            }
            62 => {
                // Dlg
                command!(self, SceCommandDlg, text: string)
            }
            63 => {
                // LoadScene
                command!(self, SceCommandLoadScene, name: string, sub_name: string)
            }
            65 => {
                // DlgSel
                command!(self, SceCommandDlgSel, list: list)
            }
            66 => {
                // GetDlgSel
                command!(self, SceCommandGetDlgSel, var: i16)
            }
            67 => {
                // DlgFace
                nop_command!(self, DlgFace, i32, string, i32)
            }
            68 => {
                // Note
                nop_command!(self, Note, string)
            }
            69 => {
                // FadeOut
                nop_command!(self, FadeOut)
            }
            70 => {
                // FadeIn
                nop_command!(self, FadeIn)
            }
            71 => {
                // RoleStop
                nop_command!(self, RoleStop, i32)
            }
            72 => {
                // RoleEmote
                nop_command!(self, RoleEmote, i32, i32)
            }
            74 => {
                // Climb
                nop_command!(self, Climb, i32, i32)
            }
            76 => {
                // DlgTime
                command!(self, SceCommandDlgTime, text: string)
            }
            77 => {
                // GetTimeSel - temporarily use GetDlgSel
                command!(self, SceCommandGetTimeSel, var: i16)
            }
            78 => {
                command!(self, SceCommandHaveItem, item_id: i32)
            }
            79 => {
                // PlaySound
                command!(self, SceCommandPlaySound, name: string, repeat: i32)
            }
            80 => {
                // CombatBoss
                nop_command!(self, CombatBoss, i32, i32, i32, i32, i32, i32)
            }
            81 => {
                // FadeWhite
                nop_command!(self, FadeWhite)
            }
            82 => {
                // CombatBoss
                nop_command!(self, CombatMaxRound, i32)
            }
            83 => {
                // CombatMustFail
                nop_command!(self, CombatMustFail)
            }
            85 => {
                // ObjectActive
                command!(self, SceCommandObjectActive, object_id: i32, active: i32)
            }
            86 => {
                // Caption
                nop_command!(self, Caption, string, i32)
            }
            87 => {
                // OpenDoor
                nop_command!(self, OpenDoor, i32)
            }
            88 => {
                // HY_Mode
                nop_command!(self, HY_Mode, i32)
            }
            89 => {
                // HY_FLY
                command!(
                    self,
                    SceCommandHyFly,
                    position_x: f32,
                    position_y: f32,
                    position_z: f32
                )
            }
            90 => {
                // ObjectMove
                nop_command!(self, ObjectMove, i32, f32, f32, f32, f32)
            }
            91 => {
                // FadeNormal
                nop_command!(self, FadeNormal)
            }
            102 => {
                // SwitchRS
                nop_command!(self, SwitchRS, i32)
            }
            104 => {
                // APPR Entry
                nop_command!(self, APPREntry)
            }
            106 => {
                // ENCAMP_Entry
                nop_command!(self, ENCAMP_Entry, i32)
            }
            107 => {
                // SKEE_Entry
                nop_command!(self, SKEE_Entry, i32)
            }
            108 => {
                // Get Appr
                command!(self, SceCommandGetAppr, var: i16)
            }
            109 => {
                // Enable_Sword
                nop_command!(self, Enable_Sword, i32)
            }
            111 => {
                // Specify_Compos
                nop_command!(self, Specify_Compos, i32)
            }
            113 => {
                // Start_HideFight
                command!(self, SceCommandStartHideFight)
            }
            115 => {
                // Movie
                command!(self, SceCommandMovie, name: string)
            }
            116 => {
                // SetRoleTexture
                nop_command!(self, SetRoleTexture, i32, string)
            }
            117 => {
                // Rotate
                nop_command!(self, Rotate, i32, i32, i32)
            }
            118 => {
                // Quake
                nop_command!(self, Quake, f32, f32)
            }
            119 => {
                // ShowChatRest
                command!(
                    self,
                    SceCommandShowChatRest,
                    config_file: string,
                    enough_money_proc: u32,
                    not_enough_money_proc: u32,
                    after_rest_proc: u32
                )
            }
            124 => {
                // Trigger
                nop_command!(self, Trigger, i32)
            }
            125 => {
                // SetBigMapElement
                command!(self, SceCommandSetBigMapElement, id: i32, option: i32)
            }
            126 => {
                // GetSwitch
                nop_command!(self, GetSwitch, string, i32, i16)
            }
            127 => {
                command!(self, SceCommandEntryRow, id: i32, proc_id: i32)
            }
            128 => {
                nop_command!(self, RotateInv, i32, i32, i32)
            }
            130 => {
                // Dist
                nop_command!(self, Dist, i16, i16)
            }
            131 => {
                // GetSwitch
                nop_command!(self, CombatNotGameOver)
            }
            132 => {
                // GetCombat
                command!(self, SceCommandGetCombat, var: i16)
            }
            133 => {
                // Music
                command!(self, SceCommandMusic, name: string, unknown: i32)
            }
            134 => {
                // StopMusic
                command!(self, SceCommandStopMusic)
            }
            135 => {
                // RoleFadeOut
                nop_command!(self, RoleFadeOut, i32)
            }
            136 => {
                // RoleFadeIn
                nop_command!(self, RoleFadeIn, i32)
            }
            137 => {
                // IfInTeam
                command!(self, SceCommandIfInTeam, role_id: i32)
            }
            138 => {
                // Enable_SwordSkill
                nop_command!(self, Enable_SwordSkill, i32)
            }
            140 => {
                // Snow
                nop_command!(self, Snow, i32)
            }
            141 => {
                // ScrEft
                nop_command!(self, ScrEft, i32)
            }
            142 => {
                // CEft_Pos
                nop_command!(self, CEft_Pos, f32, f32, f32)
            }
            143 => {
                // CEft
                nop_command!(self, CEft, i32)
            }
            144 => {
                // CEft_Role
                nop_command!(self, CEft, i32)
            }
            145 => {
                // AverageLv
                nop_command!(self, AverageLv, i32, i32)
            }
            147 => {
                // Switch2Menu
                nop_command!(self, Switch2Menu)
            }
            148 => {
                // CEft_Load
                nop_command!(self, CEft_Load, i32)
            }
            149 => {
                // GiveCloth
                nop_command!(self, GiveCloth, i16)
            }
            150 => {
                // LoadAct
                nop_command!(self, LoadAct, i32, string)
            }
            152 => {
                // WaterMagic
                nop_command!(self, WaterMagic, i32)
            }
            153 => {
                // FullTeamAtt
                nop_command!(self, FullTeamAtt)
            }
            155 => {
                // CameraYaw
                nop_command!(self, CameraYaw, f32)
            }
            156 => {
                // XJ_Pic
                nop_command!(self, XJ_Pic)
            }
            158 => {
                // ObjNotLoad
                nop_command!(self, ObjNotLoad, i32)
            }
            159 => {
                // InitFlower
                nop_command!(self, InitFlower)
            }
            201 => {
                // RolePathOut
                command!(
                    self,
                    SceCommandRolePathOut,
                    role_id: i32,
                    x: i32,
                    y: i32,
                    unknown: i32
                )
            }
            202 => {
                // InTeam
                nop_command!(self, InTeam, i32, i32)
            }
            203 => {
                // RoleSetLayer
                command!(self, SceCommandRoleSetLayer, role_id: i32, layer: i32)
            }
            204 => {
                // RoleCtrl
                command!(self, SceCommandRoleCtrl, role_id: i32)
            }
            205 => {
                // RoleOverlap
                nop_command!(self, RoleOverlap, i32, i32)
            }
            206 => {
                // RoleScale
                nop_command!(self, RoleScale, i32, f32)
            }
            207 => {
                // RoleActAutoStand
                command!(
                    self,
                    SceCommandRoleActAutoStand,
                    role_id: i32,
                    auto_play_idle: i32
                )
            }
            208 => {
                // RoleMoveBack
                command!(self, SceCommandRoleMoveBack, role_id: i32, speed: f32)
            }
            209 => {
                // RoleFaceRole
                command!(self, SceCommandRoleFaceRole, role_id: i32, role_id2: i32)
            }
            210 => {
                // RoleTurnFaceA
                command!(self, SceCommandRoleSetFace, role_id: i32, direction: i32)
            }
            211 => {
                // TeamOpenA
                nop_command!(self, TeamOpenA)
            }
            212 => {
                // TeamCloseA
                nop_command!(self, TeamCloseA)
            }
            214 => {
                // RoleMovTo
                command!(
                    self,
                    SceCommandRoleMoveTo,
                    role_id: i32,
                    x: i32,
                    y: i32,
                    unknown: i32
                )
            }
            221 => {
                // RoleEndAction
                nop_command!(self, RoleEndAction, i32)
            }
            250 => {
                // CameraFree
                nop_command!(self, CameraFree, i32)
            }
            251 => {
                // ObjectMove
                nop_command!(self, ObjectMove, i32, f32, f32, f32, f32)
            }
            default => {
                error!("Unsupported command: {}", default);
                self.put(4);
                panic!();
            }
        }
    }

    fn jump_to(&mut self, addr: u32) {
        self.program_counter = addr as usize;
    }

    fn put(&mut self, count: usize) {
        self.program_counter -= count;
    }

    fn read_string(&mut self, len: usize) -> String {
        let proc = self.sce.procs.get(&self.proc_id).unwrap();
        let end = self.program_counter + len;
        let text = encoding::all::GBK
            .decode(
                &proc.inst[self.program_counter..end - 1],
                DecoderTrap::Ignore,
            )
            .unwrap();
        self.program_counter = end;
        text
    }

    fn read(&mut self, count: usize) -> &[u8] {
        let proc = self.sce.procs.get(&self.proc_id).unwrap();
        let ret = &proc.inst[self.program_counter..self.program_counter + count];
        self.program_counter += count;
        ret
    }

    fn proc_completed(&self) -> bool {
        let proc = self.sce.procs.get(&self.proc_id).unwrap();
        self.program_counter >= proc.inst.len()
    }
}

mod data_read {
    use byteorder::{LittleEndian, ReadBytesExt};

    pub(super) fn i16(context: &mut super::SceProcContext) -> i16 {
        context.read(2).read_i16::<LittleEndian>().unwrap()
    }

    pub(super) fn i32(context: &mut super::SceProcContext) -> i32 {
        context.read(4).read_i32::<LittleEndian>().unwrap()
    }

    pub(super) fn u32(context: &mut super::SceProcContext) -> u32 {
        context.read(4).read_u32::<LittleEndian>().unwrap()
    }

    pub(super) fn f32(context: &mut super::SceProcContext) -> f32 {
        context.read(4).read_f32::<LittleEndian>().unwrap()
    }

    pub(super) fn string(context: &mut super::SceProcContext) -> String {
        let len = context.read(2).read_u16::<LittleEndian>().unwrap();
        context.read_string(len as usize)
    }

    pub(super) fn list(context: &mut super::SceProcContext) -> Vec<String> {
        let len = context.read(2).read_u16::<LittleEndian>().unwrap();
        (0..len)
            .map(|_| {
                let _ = context.read(1);
                string(context)
            })
            .collect()
    }
}

pub struct SceExecutionContext {
    sce: Rc<SceFile>,
    sce_name: String,
    proc_stack: Vec<SceProcContext>,
    options: Option<SceExecutionOptions>,
}

impl SceExecutionContext {
    pub fn new(sce: Rc<SceFile>, sce_name: String, options: Option<SceExecutionOptions>) -> Self {
        Self {
            sce,
            sce_name,
            proc_stack: vec![],
            options,
        }
    }

    pub fn set_sce(&mut self, sce: Rc<SceFile>, sce_name: String) {
        self.sce = sce;
        self.sce_name = sce_name;
    }

    pub fn call_proc(&mut self, proc_id: u32, global_state: &mut GlobalState) {
        self.proc_stack
            .push(SceProcContext::new_from_id(self.sce.clone(), proc_id));
        self.proc_begin(proc_id, global_state);
    }

    pub fn try_call_proc_by_name(&mut self, proc_name: &str, global_state: &mut GlobalState) {
        let context = SceProcContext::new_from_name(self.sce.clone(), proc_name);
        if let Some(c) = context {
            self.proc_begin(c.proc_id, global_state);
            self.proc_stack.push(c);
        }
    }

    pub fn jump_to(&mut self, addr: u32) {
        self.proc_stack.last_mut().unwrap().jump_to(addr);
    }

    pub fn set_local(&mut self, var: i16, value: i32) {
        self.proc_stack.last_mut().unwrap().set_local(var, value);
    }

    pub fn get_local(&mut self, var: i16) -> Option<i32> {
        self.proc_stack.last_mut().unwrap().get_local(var)
    }

    pub fn current_proc_context_mut(&mut self) -> &mut SceProcContext {
        self.proc_stack.last_mut().unwrap()
    }

    fn get_next_cmd(&mut self, global_state: &mut GlobalState) -> Option<Box<dyn SceCommand>> {
        while let Some(p) = self.proc_stack.last() {
            if p.proc_completed() {
                debug!("Sce proc {} completed", p.proc_id);
                self.proc_end(p.proc_id, global_state);
                self.proc_stack.pop();
            } else {
                break;
            }
        }

        self.proc_stack.last_mut().and_then(|p| p.get_next_cmd())
    }

    fn proc_begin(&self, proc_id: u32, global_state: &mut GlobalState) {
        if let Some(options) = &self.options {
            for hook in &options.proc_hooks {
                hook.proc_begin(&self.sce_name, proc_id, global_state)
            }
        }
    }

    fn proc_end(&self, proc_id: u32, global_state: &mut GlobalState) {
        if let Some(options) = &self.options {
            for hook in &options.proc_hooks {
                hook.proc_end(&self.sce_name, proc_id, global_state)
            }
        }
    }
}

pub struct SceState {
    asset_mgr: Rc<AssetManager>,
    global_state: GlobalState,
    context: SceExecutionContext,
    run_mode: i32,
    ext: HashMap<String, Box<dyn Any>>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    media_engine: Rc<dyn MediaEngine>,
}

impl SceState {
    pub fn new(
        input_engine: Rc<RefCell<dyn InputEngine>>,
        media_engine: Rc<dyn MediaEngine>,
        asset_mgr: Rc<AssetManager>,
        sce: Rc<SceFile>,
        sce_name: String,
        global_state: GlobalState,
        options: Option<SceExecutionOptions>,
    ) -> Self {
        let ext = HashMap::<String, Box<dyn Any>>::new();

        Self {
            asset_mgr: asset_mgr.clone(),
            global_state,
            context: SceExecutionContext::new(sce, sce_name, options),
            run_mode: 1,
            ext,
            input_engine,
            media_engine,
        }
    }

    pub fn global_state(&self) -> &GlobalState {
        &self.global_state
    }

    pub fn global_state_mut(&mut self) -> &mut GlobalState {
        &mut self.global_state
    }

    pub fn context_mut(&mut self) -> &mut SceExecutionContext {
        &mut self.context
    }

    pub fn input(&self) -> Ref<dyn InputEngine> {
        self.input_engine.borrow()
    }

    pub fn media_engine(&self) -> &Rc<dyn MediaEngine> {
        &self.media_engine
    }

    pub fn run_mode(&self) -> i32 {
        self.run_mode
    }

    pub fn set_run_mode(&mut self, run_mode: i32) {
        self.run_mode = run_mode;
    }

    pub fn ext_mut(&mut self) -> &mut HashMap<String, Box<dyn Any>> {
        &mut self.ext
    }

    pub fn asset_mgr(&self) -> &Rc<AssetManager> {
        &self.asset_mgr
    }

    pub fn get_next_cmd(&mut self) -> Option<Box<dyn SceCommand>> {
        self.context.get_next_cmd(&mut self.global_state)
    }

    pub fn call_proc(&mut self, proc_id: u32) {
        self.context.call_proc(proc_id, &mut self.global_state);
    }

    pub fn try_call_proc_by_name(&mut self, proc_name: &str) {
        self.context
            .try_call_proc_by_name(proc_name, &mut self.global_state);
    }
}

pub trait SceCommandDebug {
    fn debug(&self) -> String;
}

pub trait SceCommand: dyn_clone::DynClone + SceCommandDebug {
    fn initialize(&mut self, scene_manager: &mut dyn SceneManager, state: &mut SceState) {}

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool;
}

dyn_clone::clone_trait_object!(SceCommand);

impl<T: SceCommand + Debug> SceCommandDebug for T {
    fn debug(&self) -> String {
        format!("{:?}", self)
    }
}
