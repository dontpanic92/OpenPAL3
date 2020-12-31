use super::sce_commands::*;
use crate::director::sce_state::SceState;
use crate::{asset_manager::AssetManager, loaders::sce_loader::SceFile, scene::ScnScene};
use encoding::{DecoderTrap, Encoding};
use imgui::*;
use log::{debug, error};
use radiance::scene::{CoreScene, Director, Scene};
use radiance::{
    audio::{AudioEngine, AudioSourceState},
    input::InputEngine,
};
use std::rc::Rc;

pub struct SceDirector {
    asset_mgr: Rc<AssetManager>,
    vm_context: SceVmContext,
    state: SceState,
    active_commands: Vec<Box<dyn SceCommand>>,
    init: bool,
}

impl Director for SceDirector {
    fn update(&mut self, scene: &mut Box<dyn Scene>, ui: &mut Ui, delta_sec: f32) {
        let gb_scene = scene
            .as_mut()
            .downcast_mut::<CoreScene<ScnScene>>()
            .unwrap();

        if self.state.bgm_source().state() == AudioSourceState::Playing {
            self.state.bgm_source().update();
        }

        if self.state.sound_source().state() == AudioSourceState::Playing {
            self.state.sound_source().update();
        }

        if self.active_commands.len() == 0 {
            loop {
                match self.vm_context.get_next_cmd() {
                    Some(mut cmd) => {
                        cmd.initialize(gb_scene, &mut self.state);
                        if !cmd.update(gb_scene, ui, &mut self.state, delta_sec) {
                            self.active_commands.push(cmd);
                        }
                    }
                    None => break,
                };

                if self.state.run_mode() == 1 {
                    break;
                }
            }
        } else {
            let state = &mut self.state;
            self.active_commands
                .drain_filter(|cmd| cmd.update(gb_scene, ui, state, delta_sec));
        }
    }
}

impl SceDirector {
    pub fn new(
        audio_engine: &dyn AudioEngine,
        input_engine: Rc<dyn InputEngine>,
        sce: SceFile,
        entry_point: u32,
        asset_mgr: Rc<AssetManager>,
    ) -> Self {
        let state = SceState::new(audio_engine, input_engine, asset_mgr.clone());

        Self {
            asset_mgr,
            vm_context: SceVmContext::new(sce, entry_point),
            state,
            active_commands: vec![],
            init: false,
        }
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

struct SceVmContext {
    sce: SceFile,
    proc_id: u32,
    program_counter: usize,
}

impl SceVmContext {
    pub fn new(sce: SceFile, entry_point: u32) -> Self {
        let proc_id = sce
            .proc_headers
            .iter()
            .find(|h| h.id == entry_point)
            .unwrap()
            .id;

        Self {
            sce,
            proc_id,
            program_counter: 0,
        }
    }

    pub fn get_next_cmd(&mut self) -> Option<Box<dyn SceCommand>> {
        match data_read::i32(self) {
            1 => {
                // Idle
                command!(self, SceCommandIdle, length: f32)
            }
            2 => {
                // ScriptRunMode
                command!(self, SceCommandScriptRunMode, mode: i32)
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
            62 => {
                // Dlg
                command!(self, SceCommandDlg, text: string)
            }
            67 => {
                // DlgFace
                nop_command!(self, i32, string, i32)
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
                nop_command!(self, i32, i32)
            }
            88 => {
                // HY_Mode
                nop_command!(self, i32)
            }
            115 => {
                // Movie
                nop_command!(self, string)
            }
            118 => {
                // Quake
                nop_command!(self, f32, f32)
            }
            133 => {
                // Music
                command!(self, SceCommandMusic, name: string, unknown: i32)
            }
            201 => {
                // RolePathOut
                command!(
                    self,
                    SceCommandRolePathTo,
                    role_id: i32,
                    x: i32,
                    y: i32,
                    unknown: i32
                )
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
            209 => {
                // RoleFaceRole
                command!(self, SceCommandRoleFaceRole, role_id: i32, role_id2: i32)
            }
            210 => {
                // RoleTurnFaceA
                command!(self, SceCommandRoleSetFace, role_id: i32, direction: i32)
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
                None
            }
        }
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
}

mod data_read {
    use byteorder::{LittleEndian, ReadBytesExt};

    pub(super) fn i32(context: &mut super::SceVmContext) -> i32 {
        context.read(4).read_i32::<LittleEndian>().unwrap()
    }

    pub(super) fn f32(context: &mut super::SceVmContext) -> f32 {
        context.read(4).read_f32::<LittleEndian>().unwrap()
    }

    pub(super) fn string(context: &mut super::SceVmContext) -> String {
        let len = context.read(2).read_u16::<LittleEndian>().unwrap();
        context.read_string(len as usize)
    }
}

pub trait SceCommand: dyn_clone::DynClone {
    fn initialize(&mut self, scene: &mut CoreScene<ScnScene>, state: &mut SceState) {}

    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool;
}

dyn_clone::clone_trait_object!(SceCommand);
