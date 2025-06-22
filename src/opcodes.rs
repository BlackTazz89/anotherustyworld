use crate::{
    execution_context::ExecutionContext,
    vm::{Vm, VmError},
};

pub type OpcodeHandler = fn(&mut Vm, context: &mut ExecutionContext) -> Result<(), VmError>;

pub const OPCODE_TABLE: &[OpcodeHandler] = &[
    Vm::op_mov_const,
    Vm::op_mov,
    Vm::op_add,
    Vm::op_add_const,
    Vm::op_call,
    Vm::op_ret,
    Vm::op_pause_thread,
    Vm::op_jmp,
    Vm::op_set_vec,
    Vm::op_jnz,
    Vm::op_cond_jmp,
    Vm::op_set_palette,
    Vm::op_reset_threads,
    Vm::op_select_video_page,
    Vm::op_fill_video_page,
    Vm::op_copy_video_page,
    Vm::op_blit_frame_buffer,
    Vm::op_kill_thread,
    Vm::op_draw_string,
    Vm::op_sub,
    Vm::op_and,
    Vm::op_or,
    Vm::op_shl,
    Vm::op_shr,
    Vm::op_play_sound,
    Vm::op_update_mem_list,
    Vm::op_play_music,
];
