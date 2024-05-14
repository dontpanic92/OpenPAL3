use std::{cell::RefCell, rc::Rc};

use imgui::MouseButton;
use radiance::{input::Key, math::Vec3, utils::interp_value::InterpValue, video::VideoStreamState};

use crate::{
    as_params,
    openpal4::actor::Pal4ActorAnimation,
    scripting::angelscript::{
        not_implemented, ContinuationState, GlobalFunctionContinuation, GlobalFunctionState,
        ScriptGlobalContext, ScriptGlobalFunction, ScriptVm,
    },
    ui::dialog_box::{AvatarPosition, DialogBoxPresenter},
    utils,
};

use super::app_context::Pal4AppContext;

type Pal4FunctionState = GlobalFunctionState<Pal4AppContext>;
type Pal4Continuation = GlobalFunctionContinuation<Pal4AppContext>;

pub fn create_script_vm(app_context: Pal4AppContext) -> ScriptVm<Pal4AppContext> {
    let module = app_context.loader.load_script_module("script").unwrap();
    ScriptVm::new(
        Rc::new(RefCell::new(create_context())),
        module,
        0,
        app_context,
    )
}

pub fn create_context() -> ScriptGlobalContext<Pal4AppContext> {
    let mut context = ScriptGlobalContext::new();

    context.register_function(ScriptGlobalFunction::new("giIMMBegin", Box::new(imm_begin)));
    context.register_function(ScriptGlobalFunction::new("giIMMEnd", Box::new(imm_end)));
    context.register_function(ScriptGlobalFunction::new("giNewGame", Box::new(new_game)));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraCtrlYPR",
        Box::new(camera_ctrl_ypr),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraCtrlDist",
        Box::new(camera_ctrl_dist),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraCtrlYPRD",
        Box::new(camera_ctrl_yprd),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraGetDist",
        Box::new(camera_get_dist),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraGetYaw",
        Box::new(camera_get_yaw),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraGetPitch",
        Box::new(camera_get_pitch),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraGetRoll",
        Box::new(camera_get_roll),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giArenaLoad",
        Box::new(arena_load),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giArenaReady",
        Box::new(arena_ready),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giArenaReadyRestore",
        Box::new(arena_ready_restore),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giArenaHint",
        Box::new(arena_hint),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giArenaComeFromHere",
        Box::new(arena_come_from_here),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerSetLeader",
        Box::new(player_set_leader),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerSetVisible",
        Box::new(player_set_visible),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerAttachCollision",
        Box::new(player_attach_collision),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerDetachCollision",
        Box::new(player_detach_collision),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerLock",
        Box::new(player_lock),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerUnLock",
        Box::new(player_unlock),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSetNpcVisible",
        Box::new(set_npc_visible),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcCreate",
        Box::new(npc_create),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcDelete",
        Box::new(npc_delete),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSystemExchange",
        Box::new(system_exchange),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giMonsterStopPursuit",
        Box::new(monster_stop_pursuit),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraSetMode",
        Box::new(camera_set_mode),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGetGoodsOpenCondition",
        Box::new(get_goods_open_condition),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcPauseBeh",
        Box::new(npc_pause_beh),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcResumeBeh",
        Box::new(npc_resume_beh),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giOpenWeather",
        Box::new(open_weather),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCloseWeather",
        Box::new(close_weather),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSetMinimapExpmode",
        Box::new(set_minimap_expmode),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGetRandnum",
        Box::new(get_randnum),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSetTempGameState",
        Box::new(set_temp_game_state),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giFlushTailYAngle",
        Box::new(flush_tail_y_angle),
    ));

    context.register_function(ScriptGlobalFunction::new("giUnknown", Box::new(unknown)));

    context.register_function(ScriptGlobalFunction::new(
        "giAddCombatMonster",
        Box::new(add_combat_monster),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giConfigCombatParam",
        Box::new(config_combat_param),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giConfigCombatBgm",
        Box::new(config_combat_bgm),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giConfigCombatCamera",
        Box::new(config_combat_camera),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giStartCombat",
        Box::new(start_combat),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSetObjectVisible",
        Box::new(set_object_visible),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giAddProperty",
        Box::new(add_property),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giDelProperty",
        Box::new(del_property),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerInTeam",
        Box::new(player_in_team),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerOutTeam",
        Box::new(player_out_team),
    ));
    context.register_function(ScriptGlobalFunction::new("giGOMTouch", Box::new(gom_touch)));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraSetCollide",
        Box::new(camera_set_collide),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraSeekToPlayer",
        Box::new(camera_seek_to_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraAutoSeek",
        Box::new(camera_auto_seek),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerSetAttr",
        Box::new(player_set_attr),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerGetLeader",
        Box::new(player_get_leader),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSetPlayerLevel",
        Box::new(set_player_level),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giAddPlayerEquip",
        Box::new(add_player_equip),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giOpenMovieFlag",
        Box::new(open_movie_flag),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giAddQuestComplatePercentage",
        Box::new(add_quest_complete_percentage),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giAddEquipment",
        Box::new(add_equipment),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentSetVisible",
        Box::new(player_current_set_visible),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSetFullHP",
        Box::new(set_full_hp),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSetFullMP",
        Box::new(set_full_mp),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGetPlayerLevel",
        Box::new(get_player_level),
    ));
    context.register_function(ScriptGlobalFunction::new("giGotoLogo", Box::new(goto_logo)));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerUnHoldAct",
        Box::new(player_unhold_act),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcUnHoldAct",
        Box::new(npc_unhold_act),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraSetDistOptEnable",
        Box::new(camera_set_dist_opt_enable),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giMonsterSetHide",
        Box::new(monster_set_hide),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGameObjectSetResearch",
        Box::new(game_object_set_research),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSetPortrait",
        Box::new(set_portrait),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giBGMConfigSetMusic",
        Box::new(bgm_config_set_music),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giBGMConfigIsInArea",
        Box::new(bgm_config_is_in_area),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giScriptMusicMute",
        Box::new(script_music_mute),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giScriptMusicPlay",
        Box::new(script_music_play),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giScriptMusicStop",
        Box::new(script_music_stop),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giArenaMusicStop",
        Box::new(arena_music_stop),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giWorldMapSetState",
        Box::new(world_map_set_state),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGetPuzzleGameResult",
        Box::new(get_puzzle_game_result),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giAlwaysJump",
        Box::new(always_jump),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "gi2DSoundPlay",
        Box::new(sound_2d_play),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "gi2DSoundStop",
        Box::new(sound_2d_stop),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "gi2DSoundStopID",
        Box::new(sound_2d_stop_id),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCGEffPlay",
        Box::new(cg_eff_play),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCGEffStop",
        Box::new(cg_eff_stop),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectPlay",
        Box::new(effect_play),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectPlayWithPlayer",
        Box::new(effect_play_with_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectPlayWithCurrentPlayer",
        Box::new(effect_play_with_current_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectPlayWithNPC",
        Box::new(effect_play_with_npc),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectPlayWithOBJ",
        Box::new(effect_play_with_obj),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectStopWithOBJ",
        Box::new(effect_stop_with_obj),
    ));
    context.register_function(ScriptGlobalFunction::new("giShowHint", Box::new(show_hint)));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerAddSkill",
        Box::new(player_add_skill),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giMonsterSetVisible",
        Box::new(monster_set_visible),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerRandomPosition",
        Box::new(player_random_position),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentRandomPosition",
        Box::new(player_current_random_position),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEventVolumeVisible",
        Box::new(event_volume_visible),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giAllPlayerGarb2",
        Box::new(all_player_garb2),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerGarb2",
        Box::new(player_garb2),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giAllPlayerGarb1",
        Box::new(all_player_garb1),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerGarb1",
        Box::new(player_garb1),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGetVisibleObject",
        Box::new(get_visible_object),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGetVisibleMonster",
        Box::new(get_visible_monster),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCheckPackProperty",
        Box::new(check_pack_property),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGrantSystemUi",
        Box::new(grant_system_ui),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giOpenSystemUi",
        Box::new(open_system_ui),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGrantSmithSystem",
        Box::new(grant_smith_system),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGrantMagicSystem",
        Box::new(grant_magic_system),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCheckMagicMastered",
        Box::new(check_magic_mastered),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSelectDialogAddItem",
        Box::new(select_dialog_add_item),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSelectDialogGetLastSelect",
        Box::new(select_dialog_get_last_select),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGOBAttachToPlayer",
        Box::new(gob_attach_to_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGOBAttachToCurrentPlayer",
        Box::new(gob_attach_to_current_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGOBDetachFromPlayer",
        Box::new(gob_detach_from_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGOBDetachFromCurrentPlayer",
        Box::new(gob_detach_from_current_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectAttachToPlayer",
        Box::new(effect_attach_to_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectAttachToCurrentPlayer",
        Box::new(effect_attach_to_current_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectDetachFromPlayer",
        Box::new(effect_detach_from_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectDetachFromCurrentPlayer",
        Box::new(effect_detach_from_current_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectAttachToNpc",
        Box::new(effect_attach_to_npc),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEffectDetachFromNpc",
        Box::new(effect_detach_from_npc),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGOBAttachToNpc",
        Box::new(gob_attach_to_npc),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGOBDetachFromNPC",
        Box::new(gob_detach_from_npc),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGOBSetPosition",
        Box::new(gob_set_position),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giScriptClearCTXButCurrent",
        Box::new(script_clear_ctx_but_current),
    ));
    context.register_function(ScriptGlobalFunction::new("giAddMoney", Box::new(add_money)));
    context.register_function(ScriptGlobalFunction::new("giPayMoney", Box::new(pay_money)));
    context.register_function(ScriptGlobalFunction::new(
        "giHideGASkillObject",
        Box::new(hide_ga_skill_object),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giShowSignpost",
        Box::new(show_signpost),
    ));

    context.register_function(ScriptGlobalFunction::new(
        "giPlayerSetEmotion",
        Box::new(player_set_emotion),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerResetEmotion",
        Box::new(player_reset_emotion),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentSetEmotion",
        Box::new(player_current_set_emotion),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentResetEmotion",
        Box::new(player_current_reset_emotion),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giLINGSHALegsInjured",
        Box::new(lingsha_legs_injured),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giLINGSHALegsHealing",
        Box::new(lingsha_legs_healing),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcSetEmotion",
        Box::new(npc_set_emotion),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcResetEmotion",
        Box::new(npc_reset_emotion),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGetPropertyNumb",
        Box::new(get_property_numb),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentGetPosX",
        Box::new(player_current_get_pos_x),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentGetPosY",
        Box::new(player_current_get_pos_y),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentGetPosZ",
        Box::new(player_current_get_pos_z),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giArenaGetName",
        Box::new(not_implemented),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giArenaGetArea",
        Box::new(not_implemented),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giArenaSkillEnable",
        Box::new(arena_skill_enable),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giShowInnDialog",
        Box::new(show_inn_dialog),
    ));

    context.register_function(ScriptGlobalFunction::new(
        "giGetInnDialogResult",
        Box::new(get_inn_dialog_result),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerTakeARest",
        Box::new(player_take_a_rest),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giIsNightTime",
        Box::new(is_night_time),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerSetPosRot1",
        Box::new(player_set_pos_rot),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerSetPosRot2",
        Box::new(player_set_pos_rot_npc),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giStartUiTimer",
        Box::new(start_ui_timer),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerForbidenSkill",
        Box::new(player_forbiden_skill),
    ));
    context.register_function(ScriptGlobalFunction::new("giGetMoney", Box::new(get_money)));
    context.register_function(ScriptGlobalFunction::new(
        "giSelectDialogSetDefaultSelect",
        Box::new(select_dialog_set_default_select),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giShowQuestDialog",
        Box::new(show_quest_dialog),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGetQuestDialogResult",
        Box::new(get_quest_dialog_result),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giResetPlayerToJumpStart",
        Box::new(reset_player_to_jump_start),
    ));
    context.register_function(ScriptGlobalFunction::new("giGOBReset", Box::new(gob_reset)));
    context.register_function(ScriptGlobalFunction::new(
        "giCheckEquipInInventory",
        Box::new(check_equip_in_inventory),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giRemoveEquipment",
        Box::new(remove_equipment),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giAddPrescription",
        Box::new(add_prescription),
    ));

    context.register_function(ScriptGlobalFunction::new(
        "giEnableShadow",
        Box::new(enable_shadow),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giAddRoundTimes",
        Box::new(add_round_times),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giTimeScript",
        Box::new(time_script),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giTimeScriptTerminate",
        Box::new(time_script_terminate),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giConfigCombatGroundCamera",
        Box::new(config_combat_ground_camera),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giAddPlayerFavor",
        Box::new(add_player_favor),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGetPalTestResult",
        Box::new(get_pal_test_result),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSetMinimapLevel",
        Box::new(set_minimap_level),
    ));
    context.register_function(ScriptGlobalFunction::new("giPetShow", Box::new(pet_show)));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcAttachEffect",
        Box::new(npc_attach_effect),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcDetachEffect",
        Box::new(npc_detach_effect),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giMstAttachEffect",
        Box::new(mst_attach_effect),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giMstDetachEffect",
        Box::new(mst_detach_effect),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerHookEffect",
        Box::new(player_hook_effect),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerDetachEffect",
        Box::new(player_detach_effect),
    ));

    context.register_function(ScriptGlobalFunction::new(
        "giCommonDialogGetLastSelect",
        Box::new(common_dialog_get_last_select),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giConfigCombatVipMonster",
        Box::new(config_combat_vip_monster),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giEnableSTS",
        Box::new(enable_sts),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giClearUiTimer",
        Box::new(clear_ui_timer),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPauseUiTimer",
        Box::new(pause_ui_timer),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giResumeUiTimer",
        Box::new(resume_ui_timer),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giSetFullRage",
        Box::new(set_full_rage),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giUiTimerGetSaveData",
        Box::new(ui_timer_get_save_data),
    ));

    context.register_function(ScriptGlobalFunction::new(
        "giScriptMusicPause",
        Box::new(script_music_pause),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giScriptMusicResume",
        Box::new(script_music_resume),
    ));
    context.register_function(ScriptGlobalFunction::new("giWait", Box::new(wait)));
    context.register_function(ScriptGlobalFunction::new("giTalk", Box::new(talk)));

    context.register_function(ScriptGlobalFunction::new(
        "giRandTalkPush",
        Box::new(rand_talk_push),
    ));
    context.register_function(ScriptGlobalFunction::new("giRandTalk", Box::new(rand_talk)));
    context.register_function(ScriptGlobalFunction::new(
        "giRandTalkRelease",
        Box::new(rand_talk_release),
    ));
    context.register_function(ScriptGlobalFunction::new("giTalkWait", Box::new(talk_wait)));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerDoAction",
        Box::new(player_do_action),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerEndAction",
        Box::new(player_end_action),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentDoAction",
        Box::new(player_current_do_action),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentEndAction",
        Box::new(player_current_end_action),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerSetPos",
        Box::new(player_set_pos),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentSetPos",
        Box::new(player_current_set_pos),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerSetRot",
        Box::new(player_set_rot),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerSetAng",
        Box::new(player_set_ang),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentSetAng",
        Box::new(player_current_set_ang),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerFaceToPlayer",
        Box::new(player_face_to_player),
    ));

    context.register_function(ScriptGlobalFunction::new(
        "giPlayerSetDir",
        Box::new(player_set_dir),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerFaceToNpc",
        Box::new(player_face_to_npc),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerWalkTo",
        Box::new(player_walk_to),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerRunTo",
        Box::new(player_run_to),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentWalkTo",
        Box::new(player_current_walk_to),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerBackTo",
        Box::new(player_back_to),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerBlendOut",
        Box::new(player_blend_out),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerBlendIn",
        Box::new(player_blend_in),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerFaceToCurrentPlayer",
        Box::new(player_face_to_current_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCurrentPlayerFaceToNpc",
        Box::new(current_player_face_to_npc),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerDoActionRepeat",
        Box::new(player_do_action_repeat),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerEndActionRepeat",
        Box::new(player_end_action_repeat),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerEndMove",
        Box::new(player_end_move),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCurrentPlayerEndMove",
        Box::new(current_player_end_move),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcWalkTo",
        Box::new(npc_walk_to),
    ));

    context.register_function(ScriptGlobalFunction::new(
        "giNpcRunTo",
        Box::new(npc_run_to),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcBackTo",
        Box::new(npc_back_to),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcDoAction",
        Box::new(npc_do_action),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcEndAction",
        Box::new(npc_end_action),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcSetPos",
        Box::new(npc_set_pos),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcSetRot",
        Box::new(npc_set_rot),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcSetDir",
        Box::new(npc_set_dir),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcFaceToNpc",
        Box::new(npc_face_to_npc),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcFaceToPlayer",
        Box::new(npc_face_to_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcBlendOut",
        Box::new(npc_blend_out),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcBlendIn",
        Box::new(npc_blend_in),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcFaceToCurrentPlayer",
        Box::new(npc_face_to_current_player),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcResetDir",
        Box::new(npc_reset_dir),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcDoActionRepeat",
        Box::new(npc_do_action_repeat),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcEndActionRepeat",
        Box::new(npc_end_action_repeat),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcEndMove",
        Box::new(npc_end_move),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNpcSetAng",
        Box::new(npc_set_ang),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraPrepare",
        Box::new(camera_prepare),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraRunSingle",
        Box::new(camera_run_single),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraRunCircle",
        Box::new(camera_run_circle),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giCameraWait",
        Box::new(camera_wait),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giFlashOutBlack",
        Box::new(flash_out_black),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giFlashInBlack",
        Box::new(flash_in_black),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giFlashOutWhite",
        Box::new(flash_out_white),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giFlashInWhite",
        Box::new(flash_in_white),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giFlashOutRed",
        Box::new(flash_out_red),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giFlashInRed",
        Box::new(flash_in_red),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayMovie",
        Box::new(play_movie),
    ));

    context.register_function(ScriptGlobalFunction::new(
        "giObjectDoAction",
        Box::new(object_do_action),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giStartTradeSystem",
        Box::new(start_trade_system),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giStartPuzzleGame",
        Box::new(start_puzzle_game),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giStartJigsawGame",
        Box::new(start_jigsaw_game),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giOBJBlendOut",
        Box::new(obj_blend_out),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giOBJBlendIn",
        Box::new(obj_blend_in),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giMSTBlendOut",
        Box::new(mst_blend_out),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giMSTBlendIn",
        Box::new(mst_blend_in),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giShowCommonDialog",
        Box::new(show_common_dialog),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giShowSelectDialog",
        Box::new(show_select_dialog),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGOBMovment",
        Box::new(gob_movment),
    ));

    context.register_function(ScriptGlobalFunction::new(
        "giShowTutorial",
        Box::new(show_tutorial),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giShowWorldMap",
        Box::new(show_world_map),
    ));
    context.register_function(ScriptGlobalFunction::new("giGOBScale", Box::new(gob_scale)));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentFaceToGOB",
        Box::new(player_current_face_to_gob),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayerCurrentMovement",
        Box::new(player_current_movement),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giShowPoetry",
        Box::new(show_poetry),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giNPCFlyTo",
        Box::new(npc_fly_to),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giGotoLogoWait",
        Box::new(goto_logo_wait),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giShowCommonDialogInSelectMode",
        Box::new(show_common_dialog_in_select_mode),
    ));
    context.register_function(ScriptGlobalFunction::new(
        "giPlayMovieFinal",
        Box::new(play_movie_final),
    ));

    context
}

fn imm_begin(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.imm = true;
    Pal4FunctionState::Completed
}

fn imm_end(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.imm = false;
    Pal4FunctionState::Completed
}

fn new_game(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn camera_ctrl_ypr(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _yaw: f32, _pitch: f32, _roll: f32, _is_instant: i32);
    Pal4FunctionState::Completed
}

fn camera_ctrl_dist(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _dist: f32, _is_instant: i32);
    Pal4FunctionState::Completed
}

fn camera_ctrl_yprd(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _yaw: f32, _pitch: f32, _roll: f32, _dist: f32, _is_instant: i32);
    Pal4FunctionState::Completed
}

fn camera_get_dist(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<f32>(1.0);
    Pal4FunctionState::Completed
}

fn camera_get_yaw(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<f32>(1.0);
    Pal4FunctionState::Completed
}

fn camera_get_pitch(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<f32>(1.0);
    Pal4FunctionState::Completed
}

fn camera_get_roll(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<f32>(1.0);
    Pal4FunctionState::Completed
}

fn arena_load(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(
        vm,
        scn_str: i32,
        block_str: i32,
        _data_str: i32,
        _show_loading: i32
    );

    let scn = get_str(vm, scn_str as usize).unwrap();
    let block = get_str(vm, block_str as usize).unwrap();

    vm.app_context.load_scene(&scn, &block);

    let module = vm.app_context.scene.module.clone().unwrap();
    vm.set_function_by_name2(module, &format!("{}_{}_init", scn, block));

    Pal4FunctionState::Completed
}

fn arena_ready(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn arena_ready_restore(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn arena_hint(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn arena_come_from_here(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _arena_name: i32, _come_from_here_name: i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn player_set_leader(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, leader_id: i32);

    vm.app_context.set_leader(leader_id);

    Pal4FunctionState::Completed
}

fn player_set_visible(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_id: i32, _is_visible: i32);
    Pal4FunctionState::Completed
}

fn player_attach_collision(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_id: i32);
    Pal4FunctionState::Completed
}

fn player_detach_collision(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_id: i32);
    Pal4FunctionState::Completed
}

fn player_lock(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.app_context.lock_player(true);

    Pal4FunctionState::Completed
}

fn player_unlock(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.app_context.lock_player(false);

    Pal4FunctionState::Completed
}

fn set_npc_visible(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, npc_name: i32, is_visible: i32);

    let npc_name = get_str(vm, npc_name as usize).unwrap();
    vm.app_context.enable_npc(&npc_name, is_visible != 0);

    Pal4FunctionState::Completed
}

fn npc_create(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_name: i32, _behaviour_name: i32, _x_pos:f32,_y_pos:f32,_z_pos:f32);
    Pal4FunctionState::Completed
}

fn npc_delete(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_name:i32);
    Pal4FunctionState::Completed
}

fn system_exchange(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_exchange_type:i32);
    Pal4FunctionState::Completed
}

fn monster_stop_pursuit(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn camera_set_mode(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_camera_mode:i32,_is_instant:i32);
    Pal4FunctionState::Completed
}

fn get_goods_open_condition(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_goods_id:i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn npc_pause_beh(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_name:i32);
    Pal4FunctionState::Completed
}

fn npc_resume_beh(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_name:i32);
    Pal4FunctionState::Completed
}

fn open_weather(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_weather_type:i32);
    Pal4FunctionState::Completed
}

fn close_weather(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn set_minimap_expmode(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_exp_mode:i32);
    Pal4FunctionState::Completed
}

fn get_randnum(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_min:i32,_max:i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn set_temp_game_state(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_state:i32);
    Pal4FunctionState::Completed
}

fn flush_tail_y_angle(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn add_combat_monster(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_monster_id:i32,_monster_type:i32);
    Pal4FunctionState::Completed
}

fn config_combat_param(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_is_auto_fight:i32,_auto_fight_skill_id:i32,_auto_fight_skill_percent:i32,_auto_fight_skill_target_count:i32);
    Pal4FunctionState::Completed
}

fn config_combat_bgm(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_bgm_name:i32);
    Pal4FunctionState::Completed
}

fn config_combat_camera(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_camera_name:i32);
    Pal4FunctionState::Completed
}

fn start_combat(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_combat_id:i32);
    Pal4FunctionState::Completed
}

fn set_object_visible(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_object_name:i32,_is_visible:i32);
    Pal4FunctionState::Completed
}

fn add_property(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_property_id:i32,_property_value:i32,_is_persistent:i32);
    Pal4FunctionState::Completed
}

fn del_property(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_property_id:i32,_property_value:i32,_is_persistent:i32);
    Pal4FunctionState::Completed
}

fn player_in_team(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_is_in_team:i32);
    Pal4FunctionState::Completed
}

fn player_out_team(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_is_in_team:i32);
    Pal4FunctionState::Completed
}

fn gom_touch(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _file_str: i32);
    Pal4FunctionState::Completed
}

fn camera_set_collide(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _collide: i32);
    Pal4FunctionState::Completed
}

fn camera_seek_to_player(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn camera_auto_seek(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _auto_seek: i32);
    Pal4FunctionState::Completed
}

fn player_set_attr(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _attr1: i32, _attr2: i32, _attr3: i32);
    Pal4FunctionState::Completed
}

fn player_get_leader(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn set_player_level(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_id: i32, _level: i32);
    Pal4FunctionState::Completed
}

fn add_player_equip(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_id: i32, _equip_id: i32);
    Pal4FunctionState::Completed
}

fn open_movie_flag(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _flag_id: i32);
    Pal4FunctionState::Completed
}

fn add_quest_complete_percentage(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _percentage: i32);
    Pal4FunctionState::Completed
}

fn add_equipment(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _equipment_id: i32, _is_add: i32);
    Pal4FunctionState::Completed
}

fn player_current_set_visible(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _is_visible: i32);
    Pal4FunctionState::Completed
}

fn set_full_hp(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn set_full_mp(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn get_player_level(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_id: i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn goto_logo(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn player_unhold_act(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, player_id: i32);
    vm.app_context.player_unhold_act(player_id);
    Pal4FunctionState::Completed
}

fn npc_unhold_act(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_file_str:i32);
    Pal4FunctionState::Completed
}

fn camera_set_dist_opt_enable(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_enable_dist_opt :i32);
    Pal4FunctionState::Completed
}

fn monster_set_hide(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_file_str:i32,_hide_monster :i32);
    Pal4FunctionState::Completed
}

fn game_object_set_research(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_file_str:i32,_research :i32);
    Pal4FunctionState::Completed
}

fn set_portrait(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, file_str: i32, left: i32);
    let file_name = get_str(vm, file_str as usize).unwrap();
    vm.app_context.set_portrait(&file_name, left != 0);

    Pal4FunctionState::Completed
}

fn bgm_config_set_music(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_file_str:i32);
    Pal4FunctionState::Completed
}

fn bgm_config_is_in_area(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_file_str:i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn script_music_mute(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _mute: i32);
    Pal4FunctionState::Completed
}

fn script_music_play(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, str: i32, _mode: i32, _fade_in: f32, _fade_out: f32);

    let name = get_str(vm, str as usize).unwrap();
    if let Err(e) = vm.app_context.play_bgm(&name) {
        log::error!("Failed to play bgm: {}", e);
    }

    Pal4FunctionState::Completed
}

fn script_music_stop(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _flag: i32, _fade_in: f32);
    vm.app_context.stop_bgm();
    Pal4FunctionState::Completed
}

fn arena_music_stop(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _fade_out: f32);
    Pal4FunctionState::Completed
}

fn world_map_set_state(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _file_str: i32, _state_id: i32);
    Pal4FunctionState::Completed
}

fn get_puzzle_game_result(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn always_jump(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _jump: i32);
    Pal4FunctionState::Completed
}

fn sound_2d_play(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, name_str: i32, _times: i32);

    let name = get_str(vm, name_str as usize).unwrap();
    match vm.app_context.play_sound(&name) {
        Ok(id) => vm.stack_push::<i32>(id),
        Err(e) => log::error!("Failed to play sound: {}", e),
    }

    Pal4FunctionState::Completed
}

fn sound_2d_stop(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    log::warn!("sound_2d_stop is not implemented");
    Pal4FunctionState::Completed
}

fn sound_2d_stop_id(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, sound_id: i32);
    vm.app_context.stop_sound(sound_id);

    Pal4FunctionState::Completed
}

fn cg_eff_play(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _eff_id: i32);
    Pal4FunctionState::Completed
}

fn cg_eff_stop(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn effect_play(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _file_str:i32,_effect_id:i32,_x:f32,_y:f32,_z:f32);
    Pal4FunctionState::Completed
}

fn effect_play_with_player(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_file_str:i32,_effect_id:i32,_player_id:i32);
    Pal4FunctionState::Completed
}

fn effect_play_with_current_player(
    _: &str,
    vm: &mut ScriptVm<Pal4AppContext>,
) -> Pal4FunctionState {
    as_params!(vm,_file_str:i32,_effect_id:i32);
    Pal4FunctionState::Completed
}

fn effect_play_with_npc(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_file_str:i32,_effect_id:i32,_npc_file_str:i32);
    Pal4FunctionState::Completed
}

fn effect_play_with_obj(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_file_str:i32,_effect_id:i32,_obj_file_str:i32);
    Pal4FunctionState::Completed
}

fn effect_stop_with_obj(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_obj_file_str:i32);
    Pal4FunctionState::Completed
}

fn show_hint(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_hint_file_str:i32,_x:f32,_y:f32);
    Pal4FunctionState::Completed
}

fn player_add_skill(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_skill_id:i32,_add_skill:i32);
    Pal4FunctionState::Completed
}

fn monster_set_visible(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_monster_file_str:i32,_visible_monster:i32);
    Pal4FunctionState::Completed
}

fn player_random_position(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_x:f32,_y:f32);
    Pal4FunctionState::Completed
}

fn player_current_random_position(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_x:f32,_y:f32);
    Pal4FunctionState::Completed
}

fn event_volume_visible(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_event_file_str:i32,_visible_event:i32);
    Pal4FunctionState::Completed
}

fn all_player_garb2(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn player_garb2(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32);
    Pal4FunctionState::Completed
}

fn all_player_garb1(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn player_garb1(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32);
    Pal4FunctionState::Completed
}

fn get_visible_object(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_obj_file_str:i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn get_visible_monster(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_monster_file_str:i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn check_pack_property(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_property_id:i32,_property_value:i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn grant_system_ui(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_ui_id:i32,_grant_ui:i32);
    Pal4FunctionState::Completed
}

fn open_system_ui(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_ui_id:i32);
    Pal4FunctionState::Completed
}

fn grant_smith_system(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_smith1:i32,_smith2:i32,_smith3:i32,_smith4:i32);
    Pal4FunctionState::Completed
}

fn grant_magic_system(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_magic1:i32,_magic2:i32);
    Pal4FunctionState::Completed
}

fn check_magic_mastered(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn select_dialog_add_item(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_item_file_str:i32);
    Pal4FunctionState::Completed
}

fn select_dialog_get_last_select(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn gob_attach_to_player(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_gob_file_str:i32,_attach_file_str:i32,_player_id:i32,_attach_gob:i32);
    Pal4FunctionState::Completed
}

fn gob_attach_to_current_player(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_gob_file_str:i32,_attach_file_str:i32,_attach_gob:i32);
    Pal4FunctionState::Completed
}

fn gob_detach_from_player(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32);
    Pal4FunctionState::Completed
}

fn gob_detach_from_current_player(
    _: &str,
    _vm: &mut ScriptVm<Pal4AppContext>,
) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn effect_attach_to_player(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_effect_file_str:i32,_attach_effect:i32);
    Pal4FunctionState::Completed
}

fn effect_attach_to_current_player(
    _: &str,
    vm: &mut ScriptVm<Pal4AppContext>,
) -> Pal4FunctionState {
    as_params!(vm,_effect_file_str:i32,_attach_effect:i32);
    Pal4FunctionState::Completed
}

fn effect_detach_from_player(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32);
    Pal4FunctionState::Completed
}

fn effect_detach_from_current_player(
    _: &str,
    _vm: &mut ScriptVm<Pal4AppContext>,
) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn effect_attach_to_npc(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str: i32, _effect_file_str: i32, _attach_effect: i32);
    Pal4FunctionState::Completed
}

fn effect_detach_from_npc(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str: i32);
    Pal4FunctionState::Completed
}

fn gob_attach_to_npc(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _gob_file_str: i32, _attach_file_str: i32, _npc_file_str: i32, _attach_gob: i32);
    Pal4FunctionState::Completed
}

fn gob_detach_from_npc(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str: i32);
    Pal4FunctionState::Completed
}

fn gob_set_position(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _gob_file_str: i32, _x: f32, _y: f32, _z: f32);
    Pal4FunctionState::Completed
}

fn script_clear_ctx_but_current(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn add_money(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _money_amount: i32, _add_money: i32);
    Pal4FunctionState::Completed
}

fn pay_money(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _money_amount: i32, _pay_money: i32);
    Pal4FunctionState::Completed
}

fn hide_ga_skill_object(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn show_signpost(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn player_set_emotion(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_id: i32, _emotion_file_str: i32);
    Pal4FunctionState::Completed
}

fn player_reset_emotion(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_id: i32);
    Pal4FunctionState::Completed
}

fn player_current_set_emotion(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _emotion_file_str: i32);
    Pal4FunctionState::Completed
}

fn player_current_reset_emotion(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn lingsha_legs_injured(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _injured_file_str: i32);
    Pal4FunctionState::Completed
}

fn lingsha_legs_healing(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn npc_set_emotion(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str:i32,_emotion_file_str:i32);
    Pal4FunctionState::Completed
}

fn npc_reset_emotion(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_file_str:i32);
    Pal4FunctionState::Completed
}

fn get_property_numb(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_property_id:i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn player_current_get_pos_x(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<f32>(1.0);
    Pal4FunctionState::Completed
}

fn player_current_get_pos_y(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<f32>(1.0);
    Pal4FunctionState::Completed
}

fn player_current_get_pos_z(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<f32>(1.0);
    Pal4FunctionState::Completed
}

fn arena_skill_enable(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _enable: i32);
    Pal4FunctionState::Completed
}

fn show_inn_dialog(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _show: i32);
    Pal4FunctionState::Completed
}

fn get_inn_dialog_result(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn player_take_a_rest(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn is_night_time(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn player_set_pos_rot(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_id: i32, _x: f32, _y: f32, _z: f32, _rot: f32);
    Pal4FunctionState::Completed
}

fn player_set_pos_rot_npc(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_file_str: i32, _x: f32, _y: f32, _z: f32, _rot: f32);
    Pal4FunctionState::Completed
}

fn start_ui_timer(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _timer_id: i32, _timer_file_str: i32);
    Pal4FunctionState::Completed
}

fn player_forbiden_skill(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_id: i32, _skill_id: i32, _forbiden_skill: i32);
    Pal4FunctionState::Completed
}

fn get_money(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn select_dialog_set_default_select(
    _: &str,
    vm: &mut ScriptVm<Pal4AppContext>,
) -> Pal4FunctionState {
    as_params!(vm, _select_id: i32);
    Pal4FunctionState::Completed
}

fn show_quest_dialog(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_quest_file_str:i32);
    Pal4FunctionState::Completed
}

fn get_quest_dialog_result(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn reset_player_to_jump_start(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn gob_reset(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_gob_file_str:i32);
    Pal4FunctionState::Completed
}

fn check_equip_in_inventory(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_equip_id:i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn remove_equipment(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_equip_id:i32,_remove_equip :i32);
    Pal4FunctionState::Completed
}

fn add_prescription(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_prescription_id:i32,_add_prescription :i32);
    Pal4FunctionState::Completed
}

fn enable_shadow(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_enable_shadow :i32);
    Pal4FunctionState::Completed
}

fn add_round_times(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn time_script(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_time:f32,_script_file_str:i32);
    Pal4FunctionState::Completed
}

fn time_script_terminate(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn config_combat_ground_camera(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_camera_file_str:i32);
    Pal4FunctionState::Completed
}

fn add_player_favor(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_favor_id:i32,_favor_value:i32);
    Pal4FunctionState::Completed
}

fn get_pal_test_result(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_test_id:i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn set_minimap_level(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_level_id:i32);
    Pal4FunctionState::Completed
}

fn pet_show(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_show_pet:i32);
    Pal4FunctionState::Completed
}

fn npc_attach_effect(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_file_str:i32,_effect_file_str:i32,_effect_id:i32);
    Pal4FunctionState::Completed
}

fn npc_detach_effect(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_file_str:i32);
    Pal4FunctionState::Completed
}

fn mst_attach_effect(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_mst_file_str:i32,_effect_file_str:i32,_effect_id:i32);
    Pal4FunctionState::Completed
}

fn mst_detach_effect(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_mst_file_str:i32);
    Pal4FunctionState::Completed
}

fn player_hook_effect(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_effect_file_str:i32,_effect_id:i32);
    Pal4FunctionState::Completed
}

fn player_detach_effect(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32);
    Pal4FunctionState::Completed
}

fn common_dialog_get_last_select(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn config_combat_vip_monster(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _monster_id: i32);
    Pal4FunctionState::Completed
}

fn enable_sts(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _sts_id: i32);
    Pal4FunctionState::Completed
}

fn clear_ui_timer(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn pause_ui_timer(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn resume_ui_timer(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn set_full_rage(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn ui_timer_get_save_data(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn script_music_pause(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn script_music_resume(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn wait(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, time: f64);

    let mut time = time as f32;
    Pal4FunctionState::Yield(Box::new(move |vm, delta_sec| {
        if time <= 0.0 {
            ContinuationState::Completed
        } else {
            time = time - delta_sec;
            ContinuationState::Loop
        }
    }))
}

fn talk(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, str: i32);
    let voice = vm.stack_peek::<i32>();

    if let Some(voice) = voice {
        let voice_name = vm.heap.get(voice as usize).cloned().flatten();
        if let Some(voice_name) = voice_name {
            if voice_name.len() == 5 && voice_name.starts_with("4") {
                if let Err(e) = vm.app_context.play_voice(&voice_name) {
                    log::debug!("Play voice failed: {}", e);
                }
            }
        }
    }

    let text = get_str(vm, str as usize).unwrap();
    vm.app_context.dialog_box.set_text(&text);
    let presenter = DialogBoxPresenter::new();

    Pal4FunctionState::Yield(Box::new(move |vm, delta_sec| {
        let ui = &vm.app_context().ui;
        presenter.update(&vm.app_context.dialog_box, delta_sec);

        let input = vm.app_context().input.clone();
        let input = input.borrow();
        let completed = ui.ui().is_mouse_released(MouseButton::Left)
            || input.get_key_state(Key::GamePadEast).pressed()
            || input.get_key_state(Key::GamePadSouth).pressed()
            || input.get_key_state(Key::Space).pressed();
        if completed {
            vm.app_context
                .dialog_box
                .set_avatar(None, AvatarPosition::Left);
            ContinuationState::Completed
        } else {
            ContinuationState::Loop
        }
    }))
}

fn rand_talk_push(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _talk_file_str: i32);
    Pal4FunctionState::Completed
}

fn rand_talk(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn rand_talk_release(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn talk_wait(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn player_do_action(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, player: i32, action_str: i32, flag: i32, _sync: i32);

    let action = get_str(vm, action_str as usize).unwrap();
    vm.app_context.player_do_action(player, &action, flag);

    Pal4FunctionState::Completed
}

fn player_end_action(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, player_id: i32);

    Pal4FunctionState::Yield(Box::new(move |vm, _delta_sec| {
        if vm.app_context.player_act_completed(player_id) {
            ContinuationState::Completed
        } else {
            ContinuationState::Loop
        }
    }))
}

fn player_current_do_action(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _action_file_str:i32,_action_id:i32,_do_action :i32);
    Pal4FunctionState::Completed
}

fn player_current_end_action(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn player_set_pos(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_x:f32,_y:f32,_z:f32);
    Pal4FunctionState::Completed
}

fn player_current_set_pos(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, x:f32, y:f32, z:f32);
    vm.app_context.set_player_pos(-1, &Vec3::new(x, y, z));
    Pal4FunctionState::Completed
}

fn player_set_rot(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _player_id: i32, rot_file_str: i32);
    let rot_file = get_str(vm, rot_file_str as usize).unwrap();
    println!("rot_file {}", rot_file);

    Pal4FunctionState::Completed
}

fn player_set_ang(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_ang:f32);
    Pal4FunctionState::Completed
}

fn player_current_set_ang(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, ang: f32);

    vm.app_context.set_player_ang(-1, ang);

    Pal4FunctionState::Completed
}

fn player_face_to_player(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player1_id:i32,_player2_id:i32,_face_to_player :i32);
    Pal4FunctionState::Completed
}

fn player_set_dir(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, player_id: i32, direction: f32, _sync :i32);

    vm.app_context.player_set_direction(player_id, direction);
    Pal4FunctionState::Completed
}

fn player_face_to_npc(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_npc_file_str:i32,_face_to_npc :i32);
    Pal4FunctionState::Completed
}

fn player_walk_to(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, player_id:i32, x:f32, y:f32, z:f32, _walk_to :i32);

    vm.app_context
        .player_to(player_id, &Vec3::new(x, y, z), false);
    Pal4FunctionState::Completed
}

fn player_run_to(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, player_id:i32, x:f32, y:f32, z:f32, _run_to :i32);

    vm.app_context
        .player_to(player_id, &Vec3::new(x, y, z), true);
    Pal4FunctionState::Completed
}

fn player_current_walk_to(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_x:f32,_y:f32,_z:f32,_walk_to :i32);
    Pal4FunctionState::Completed
}

fn player_back_to(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_x:f32,_y:f32,_z:f32,_back_to :i32);
    Pal4FunctionState::Completed
}

fn player_blend_out(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_blend_out_time:f32,_blend_out :i32);
    Pal4FunctionState::Completed
}

fn player_blend_in(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_blend_in_time:f32,_blend_in :i32);
    Pal4FunctionState::Completed
}

fn player_face_to_current_player(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32,_face_to_current_player :i32);
    Pal4FunctionState::Completed
}

fn current_player_face_to_npc(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_file_str:i32,_face_to_npc :i32);
    Pal4FunctionState::Completed
}

fn player_do_action_repeat(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, player: i32, action_str: i32);
    let action = get_str(vm, action_str as usize).unwrap();
    vm.app_context.player_do_action(player, &action, 0);

    Pal4FunctionState::Completed
}

fn player_end_action_repeat(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_player_id:i32);
    Pal4FunctionState::Completed
}

fn player_end_move(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, player_id:i32);

    Pal4FunctionState::Yield(Box::new(move |vm, _delta_sec| {
        if vm.app_context.player_moving(player_id) {
            ContinuationState::Loop
        } else {
            ContinuationState::Completed
        }
    }))
}

fn current_player_end_move(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Yield(Box::new(move |vm, _delta_sec| {
        if vm.app_context.player_moving(-1) {
            ContinuationState::Loop
        } else {
            ContinuationState::Completed
        }
    }))
}

fn npc_walk_to(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, npc_file_str: i32, x: f32, y: f32, z: f32, _walk_to: i32);

    let npc_name = get_str(vm, npc_file_str as usize).unwrap();
    vm.app_context.npc_to(&npc_name, &Vec3::new(x, y, z), false);
    Pal4FunctionState::Completed
}

fn npc_run_to(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, npc_file_str: i32, x: f32, y: f32, z: f32, _run_to: i32);

    let npc_name = get_str(vm, npc_file_str as usize).unwrap();
    vm.app_context.npc_to(&npc_name, &Vec3::new(x, y, z), true);
    Pal4FunctionState::Completed
}

fn npc_back_to(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str: i32, _x: f32, _y: f32, _z: f32, _back_to: i32);
    Pal4FunctionState::Completed
}

fn npc_do_action(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str: i32, _action_file_str: i32, _action_id: i32, _do_action: i32);
    Pal4FunctionState::Completed
}

fn npc_end_action(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str: i32, _end_action: i32);
    Pal4FunctionState::Completed
}

fn npc_set_pos(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str: i32, _x: f32, _y: f32, _z: f32);
    Pal4FunctionState::Completed
}

fn npc_set_rot(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str: i32, _rot_file_str: i32);
    Pal4FunctionState::Completed
}

fn npc_set_dir(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str: i32, _dir: f32, _set_dir: i32);
    Pal4FunctionState::Completed
}

fn npc_face_to_npc(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc1_file_str: i32, _npc2_file_str: i32, _face_to_npc: i32);
    Pal4FunctionState::Completed
}

fn npc_face_to_player(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str: i32, _player_id: i32, _face_to_player: i32);
    Pal4FunctionState::Completed
}

fn npc_blend_out(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _npc_file_str: i32, _blend_out_time: f32, _blend_out: i32);
    Pal4FunctionState::Completed
}

fn npc_blend_in(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_file_str:i32,_blend_in_time:f32,_blend_in :i32);
    Pal4FunctionState::Completed
}

fn npc_face_to_current_player(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_file_str:i32,_face_to_current_player :i32);
    Pal4FunctionState::Completed
}

fn npc_reset_dir(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_file_str:i32);
    Pal4FunctionState::Completed
}

fn npc_do_action_repeat(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_file_str:i32,_action_file_str:i32);
    Pal4FunctionState::Completed
}

fn npc_end_action_repeat(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_file_str:i32);
    Pal4FunctionState::Completed
}

fn npc_end_move(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, npc_file_str:i32);

    let npc_name = get_str(vm, npc_file_str as usize).unwrap();

    Pal4FunctionState::Yield(Box::new(move |vm, _delta_sec| {
        if vm.app_context.npc_moving(&npc_name) {
            ContinuationState::Loop
        } else {
            ContinuationState::Completed
        }
    }))
}

fn npc_set_ang(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_file_str:i32,_ang:f32);
    Pal4FunctionState::Completed
}

fn camera_prepare(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, file_str: i32);
    let file_name = get_str(vm, file_str as usize).unwrap();

    if let Err(e) = vm.app_context.prepare_camera(&file_name) {
        log::error!("camera prepare failed: {}", e);
        vm.stack_push::<i32>(0);
    } else {
        vm.stack_push::<i32>(1);
    }

    Pal4FunctionState::Completed
}

fn camera_run_single(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, camera_data: i32, _sync: i32);
    let name = get_str(vm, camera_data as usize).unwrap();

    vm.app_context.run_camera(&name);

    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn camera_run_circle(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_run_circle :i32);
    vm.stack_push::<i32>(1);
    Pal4FunctionState::Completed
}

fn camera_wait(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn flash_out_black(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, duration: f32, keep: i32, sync: i32);

    vm.app_context
        .set_actdrop(InterpValue::new(0., 1., duration));

    if sync == 1 {
        Pal4FunctionState::Yield(Box::new(move |vm, _| {
            if vm.app_context.get_actdrop().current() == 1. {
                ContinuationState::Completed
            } else {
                ContinuationState::Loop
            }
        }))
    } else {
        Pal4FunctionState::Completed
    }
}

fn flash_in_black(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, duration: f32, sync: i32);

    vm.app_context
        .set_actdrop(InterpValue::new(1., 0., duration));

    if sync == 1 {
        Pal4FunctionState::Yield(Box::new(move |vm, _| {
            if vm.app_context.get_actdrop().current() == 0. {
                ContinuationState::Completed
            } else {
                ContinuationState::Loop
            }
        }))
    } else {
        Pal4FunctionState::Completed
    }
}

fn flash_out_white(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _flash_time:f32,_flash_out_white1 :i32,_flash_out_white2 :i32);
    Pal4FunctionState::Completed
}

fn flash_in_white(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _flash_time:f32,_flash_in_white :i32);
    Pal4FunctionState::Completed
}

fn flash_out_red(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _flash_time:f32,_flash_out_red1 :i32,_flash_out_red2 :i32);
    Pal4FunctionState::Completed
}

fn flash_in_red(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _flash_time:f32,_flash_in_red1 :i32,_flash_in_red2 :i32);
    Pal4FunctionState::Completed
}

const MOVIES_CONTAIN_BLACK_BARS: &[&str; 1] = &["pal4a.bik"];

fn play_movie(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, movie_file_str:i32);

    let movie_name = get_str(vm, movie_file_str as usize).unwrap();
    let source_size = match vm.app_context.start_play_movie(&movie_name) {
        Some(size) => size,
        None => {
            log::warn!("Skip movie '{}'", movie_name);
            return Pal4FunctionState::Completed;
        }
    };
    let mut texture_id = None;
    let remove_black_bars = MOVIES_CONTAIN_BLACK_BARS
        .iter()
        .any(|&name| movie_name.to_lowercase().as_str() == name);

    Pal4FunctionState::Yield(Box::new(move |vm, _| {
        let ui = vm.app_context.ui.clone();

        let movie_skipped = {
            let input = vm.app_context().input.borrow();
            input.get_key_state(Key::Escape).pressed()
                || input.get_key_state(Key::GamePadSouth).pressed()
        };

        let video_player = vm.app_context.video_player();
        if movie_skipped {
            video_player.stop();
            return ContinuationState::Completed;
        }
        if video_player.get_state() == VideoStreamState::Stopped {
            return ContinuationState::Completed;
        }

        texture_id = utils::play_movie(
            ui.ui(),
            video_player,
            texture_id,
            source_size,
            remove_black_bars,
        );

        ContinuationState::Loop
    }))
}

fn object_do_action(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_object_file_str:i32,_action_file_str:i32,_action_id:i32,_do_action :i32);
    Pal4FunctionState::Completed
}

fn start_trade_system(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_trade_file_str1:i32,_trade_file_str2:i32);
    Pal4FunctionState::Completed
}

fn start_puzzle_game(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_puzzle_id:i32);
    Pal4FunctionState::Completed
}

fn start_jigsaw_game(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_jigsaw_id:i32);
    Pal4FunctionState::Completed
}

fn obj_blend_out(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_obj_file_str:i32,_blend_out_time:f32,_blend_out :i32);
    Pal4FunctionState::Completed
}

fn obj_blend_in(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_obj_file_str:i32,_blend_in_time:f32,_blend_in :i32);
    Pal4FunctionState::Completed
}

fn mst_blend_out(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_mst_file_str:i32,_blend_out_time:f32,_blend_out :i32);
    Pal4FunctionState::Completed
}

fn mst_blend_in(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_mst_file_str:i32,_blend_in_time:f32,_blend_in :i32);
    Pal4FunctionState::Completed
}

fn show_common_dialog(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_dialog_file_str:i32,_x:f32,_y:f32);
    Pal4FunctionState::Completed
}

fn show_select_dialog(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_dialog_file_str:i32);
    Pal4FunctionState::Completed
}
fn gob_movment(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _gob_file_str: i32, _x: f32, _y: f32, _z: f32, _rot: f32, _movment: i32);
    Pal4FunctionState::Completed
}

fn show_tutorial(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _key_id: i32);
    Pal4FunctionState::Completed
}

fn show_world_map(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn gob_scale(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _gob_file_str: i32, _x_scale: f32, _y_scale: f32, _scale_gob: i32);
    Pal4FunctionState::Completed
}

fn player_current_face_to_gob(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_gob_file_str:i32,_face_to_gob :i32);
    Pal4FunctionState::Completed
}

fn player_current_movement(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_x:f32,_y:f32,_z:f32,_rot:f32,_movment :i32);
    Pal4FunctionState::Completed
}

fn show_poetry(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_poetry_id:i32,_show_poetry :i32);
    Pal4FunctionState::Completed
}

fn npc_fly_to(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm,_npc_file_str:i32,_x:f32,_y:f32,_z:f32,_fly_to :i32);
    Pal4FunctionState::Completed
}

fn goto_logo_wait(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn show_common_dialog_in_select_mode(
    _: &str,
    vm: &mut ScriptVm<Pal4AppContext>,
) -> Pal4FunctionState {
    as_params!(vm, _dialog_file_str:i32,_x:f32,_y:f32);
    Pal4FunctionState::Completed
}

fn play_movie_final(_: &str, vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    as_params!(vm, _movie_file_str:i32);
    Pal4FunctionState::Completed
}

fn unknown(_: &str, _vm: &mut ScriptVm<Pal4AppContext>) -> Pal4FunctionState {
    Pal4FunctionState::Completed
}

fn get_str(vm: &mut ScriptVm<Pal4AppContext>, index: usize) -> Option<String> {
    vm.heap[index].clone()
}
