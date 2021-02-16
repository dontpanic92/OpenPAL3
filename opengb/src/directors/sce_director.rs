use super::{
    exp_director::ExplorationDirector, sce_commands::*, shared_state::SharedState,
};
use crate::{asset_manager::AssetManager, loaders::sce_loader::SceFile};
use encoding::{DecoderTrap, Encoding};
use imgui::*;
use log::{debug, error};
use radiance::scene::{Director, SceneManager};
use radiance::{audio::AudioEngine, input::InputEngine};
use std::{
    any::Any,
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    rc::{Rc, Weak},
};

pub struct SceDirector {
    shared_self: Weak<RefCell<Self>>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    state: SceState,
    shared_state: Rc<RefCell<SharedState>>,
    active_commands: Vec<Box<dyn SceCommand>>,
}

impl Director for SceDirector {
    fn activate(&mut self, scene_manager: &mut dyn SceneManager) {
        debug!("SceDirector activated");
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        self.shared_state.borrow_mut().update(delta_sec);

        if self.active_commands.len() == 0 {
            loop {
                match self.state.vm_context.get_next_cmd() {
                    Some(mut cmd) => {
                        cmd.initialize(scene_manager, &mut self.state);
                        if !cmd.update(scene_manager, ui, &mut self.state, delta_sec) {
                            self.active_commands.push(cmd);
                        }
                    }
                    None => {
                        return Some(Rc::new(RefCell::new(ExplorationDirector::new(
                            self.shared_self.upgrade().unwrap(),
                            self.input_engine.clone(),
                            self.shared_state.clone(),
                        ))));
                    }
                };

                if self.state.run_mode() == 1 {
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
}

impl SceDirector {
    pub fn new(
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        sce: SceFile,
        asset_mgr: Rc<AssetManager>,
        shared_state: Rc<RefCell<SharedState>>,
    ) -> Rc<RefCell<Self>> {
        let state = SceState::new(
            input_engine.clone(),
            audio_engine.clone(),
            asset_mgr.clone(),
            Rc::new(sce),
            shared_state.clone(),
        );
        let director = Rc::new(RefCell::new(Self {
            shared_self: Weak::new(),
            input_engine,
            state,
            shared_state,
            active_commands: vec![],
        }));

        director.borrow_mut().shared_self = Rc::downgrade(&director);
        director
    }

    pub fn call_proc(&mut self, proc_id: u32) {
        self.state.vm_context.call_proc(proc_id)
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
            debug!(concat!("{} ", $(concat!("{", stringify!($asc_param_names), "} "), )*), stringify!($cmd_name), $($asc_param_names=$asc_param_names, )*);
            Some(Box::new($cmd_name::new($($asc_param_names),*)))
        }
    };
}

macro_rules! nop_command {
    ($self: ident $(, $param_types: ident)* $(,)*) => {
        nop_command! {@reverse_arg $self $(, $param_types)* []}
    };

    (@reverse_arg $self: ident, $param_type: ident $(, $param_types: ident)* [$(, $reversed_types: ident)*]) => {
        nop_command! {@reverse_arg $self $(, $param_types)* [, $param_type $(, $reversed_types)*]}
    };

    (@reverse_arg $self: ident [$(, $reversed_types: ident)*]) => {
        nop_command! {@inner $self $(, $reversed_types)*}
    };

    (@inner $self: ident $(, $param_types: ident)*) => {
        {
            $(let _ = data_read::$param_types($self); )*
            Some(Box::new(SceCommandNop::new()))
        }
    };
}

struct SceProcContext {
    sce: Rc<SceFile>,
    proc_id: u32,
    program_counter: usize,
    local_vars: HashMap<i16, i32>,
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
            .position(|h| h.name == proc_name)
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
        }
    }

    pub fn set_local(&mut self, var: i16, value: i32) {
        self.local_vars.insert(var, value);
    }

    pub fn get_local(&mut self, var: i16) -> Option<i32> {
        self.local_vars.get(&var).and_then(|v| Some(*v))
    }

    pub fn get_next_cmd(&mut self) -> Option<Box<dyn SceCommand>> {
        if self.proc_completed() {
            return None;
        }

        match data_read::i32(self) {
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
            6 | 65542 => {
                // GT
                command!(self, SceCommandGt, var: i16, value: i32)
            }
            7 | 65543 => {
                // LS
                command!(self, SceCommandLs, var: i16, value: i32)
            }
            8 | 65544 => {
                // EQ
                command!(self, SceCommandEq, var: i16, value: i32)
            }
            9 | 65545 => {
                // NEQ
                command!(self, SceCommandNeq, var: i16, value: i32)
            }
            10 | 65546 => {
                // GEQ
                command!(self, SceCommandGeq, var: i16, value: i32)
            }
            11 | 65547 => {
                // LEQ
                command!(self, SceCommandLeq, var: i16, value: i32)
            }
            12 => {
                // TestGoto
                command!(self, SceCommandTestGoto, offset: u32)
            }
            13 | 65549 => {
                // Let
                command!(self, SceCommandLet, var: i16, value: i32)
            }
            16 => {
                //Call
                command!(self, SceCommandCall, proc_id: u32)
            }
            17 | 65553 => {
                // Rnd
                command!(self, SceCommandRnd, var: i16, value: i32)
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
            27 => {
                // RoleInput
                nop_command!(self, i32)
            }
            28 => {
                // RoleActive
                command!(self, SceCommandRoleActive, role: i32, active: i32)
            }
            32 => {
                // CameraPush
                nop_command!(self, f32, f32, i32)
            }
            33 => {
                // CameraRotate
                nop_command!(self, f32, f32, f32, i32)
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
            46 => {
                // AddItem
                nop_command!(self, i32, i32)
            }
            62 => {
                // Dlg
                command!(self, SceCommandDlg, text: string)
            }
            63 => {
                // LoadScene
                command!(self, SceCommandLoadScene, name: string, sub_name: string)
            }
            67 => {
                // DlgFace
                nop_command!(self, i32, string, i32)
            }
            68 => {
                // Note
                nop_command!(self, string)
            }
            69 => {
                // FadeOut
                nop_command!(self)
            }
            70 => {
                // FadeIn
                nop_command!(self)
            }
            71 => {
                // RoleStop
                nop_command!(self, i32)
            }
            72 => {
                // RoleEmote
                nop_command!(self, i32, i32)
            }
            79 => {
                // PlaySound
                command!(self, SceCommandPlaySound, name: string, repeat: i32)
            }
            85 => {
                // ObjectActive
                command!(self, SceCommandObjectActive, object_id: i32, active: i32)
            }
            86 => {
                // Caption
                nop_command!(self, string, i32)
            }
            88 => {
                // HY_Mode
                nop_command!(self, i32)
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
            104 => {
                // APPR Entry
                nop_command!(self)
            }
            108 | 65644 => {
                // Get Appr
                command!(self, SceCommandGetAppr, var: i16)
            }
            115 => {
                // Movie
                nop_command!(self, string)
            }
            116 => {
                // SetRoleTexture
                nop_command!(self, i32, string)
            }
            118 => {
                // Quake
                nop_command!(self, f32, f32)
            }
            124 => {
                // Trigger
                nop_command!(self, i32)
            }
            133 => {
                // Music
                command!(self, SceCommandMusic, name: string, unknown: i32)
            }
            134 => {
                // StopMusic
                command!(self, SceCommandStopMusic)
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
                nop_command!(self, i32, i32)
            }
            204 => {
                // RoleCtrl
                nop_command!(self, i32)
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
                nop_command!(self, i32)
            }
            250 => {
                // CameraFree
                nop_command!(self, i32)
            }
            default => {
                error!("Unsupported command: {}", default);
                self.put(4);
                panic!();
            }
        }
    }

    pub fn jump_to(&mut self, addr: u32) {
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
}

pub struct SceVmContext {
    sce: Rc<SceFile>,
    proc_stack: Vec<SceProcContext>,
}

impl SceVmContext {
    pub fn new(sce: Rc<SceFile>) -> Self {
        Self {
            sce,
            proc_stack: vec![],
        }
    }

    pub fn set_sce(&mut self, sce: Rc<SceFile>) {
        self.sce = sce;
    }

    pub fn call_proc(&mut self, proc_id: u32) {
        self.proc_stack
            .push(SceProcContext::new_from_id(self.sce.clone(), proc_id))
    }

    pub fn try_call_proc_by_name(&mut self, proc_name: &str) {
        let context = SceProcContext::new_from_name(self.sce.clone(), proc_name);
        if let Some(c) = context {
            self.proc_stack.push(c)
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

    pub fn get_next_cmd(&mut self) -> Option<Box<dyn SceCommand>> {
        while let Some(p) = self.proc_stack.last() {
            if p.proc_completed() {
                debug!("Sce proc {} completed", p.proc_id);
                self.proc_stack.pop();
            } else {
                break;
            }
        }

        self.proc_stack.last_mut().and_then(|p| p.get_next_cmd())
    }
}

pub struct SceState {
    asset_mgr: Rc<AssetManager>,
    shared_state: Rc<RefCell<SharedState>>,
    fop_state: FopState,
    vm_context: SceVmContext,
    run_mode: i32,
    ext: HashMap<String, Box<dyn Any>>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    audio_engine: Rc<dyn AudioEngine>,
}

impl SceState {
    pub fn new(
        input_engine: Rc<RefCell<dyn InputEngine>>,
        audio_engine: Rc<dyn AudioEngine>,
        asset_mgr: Rc<AssetManager>,
        sce: Rc<SceFile>,
        shared_state: Rc<RefCell<SharedState>>,
    ) -> Self {
        let ext = HashMap::<String, Box<dyn Any>>::new();

        Self {
            asset_mgr: asset_mgr.clone(),
            shared_state,
            fop_state: FopState::new(),
            vm_context: SceVmContext::new(sce),
            run_mode: 1,
            ext,
            input_engine,
            audio_engine,
        }
    }

    pub fn shared_state_mut(&mut self) -> RefMut<SharedState> {
        self.shared_state.borrow_mut()
    }

    pub fn fop_state_mut(&mut self) -> &mut FopState {
        &mut self.fop_state
    }

    pub fn vm_context_mut(&mut self) -> &mut SceVmContext {
        &mut self.vm_context
    }

    pub fn input(&self) -> Ref<dyn InputEngine> {
        self.input_engine.borrow()
    }

    pub fn audio(&self) -> &Rc<dyn AudioEngine> {
        &self.audio_engine
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
}

pub enum Fop {
    And,
    Or,
}

pub struct FopState {
    lhs: Option<bool>,
    op: Option<Fop>,
}

impl FopState {
    pub fn new() -> Self {
        Self {
            lhs: None,
            op: None,
        }
    }

    pub fn push_value(&mut self, value: bool) {
        self.lhs = match (&self.lhs, &self.op) {
            (Some(lhs), Some(Fop::And)) => Some(*lhs && value),
            (Some(lhs), Some(Fop::Or)) => Some(*lhs || value),
            (None, _) => Some(value),
            _ => panic!("Fop State error - might be a bug in Sce"),
        }
    }

    pub fn set_op(&mut self, op: Fop) {
        self.op = Some(op);
    }

    pub fn reset(&mut self) {
        self.lhs = None;
        self.op = None;
    }

    pub fn value(&self) -> Option<bool> {
        self.lhs
    }
}

pub trait SceCommand: dyn_clone::DynClone {
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
