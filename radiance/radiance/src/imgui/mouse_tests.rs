use super::*;

fn frame(
    context: &mut imgui::Context,
    mouse: &mut SyntheticMouseInput,
) -> ([f32; 2], bool, bool, bool, [f32; 2]) {
    mouse.apply_frame(context.io_mut());
    let ui = context.frame();
    let state = (
        ui.io().mouse_pos,
        ui.is_mouse_down(MouseButton::Left),
        ui.is_mouse_clicked(MouseButton::Left),
        ui.is_mouse_released(MouseButton::Left),
        [ui.io().mouse_wheel_h, ui.io().mouse_wheel],
    );
    context.render();
    state
}

#[test]
fn synthetic_mouse_click_drag_scroll_and_reset_reach_imgui_in_order() {
    let mut context = imgui::Context::create();
    context.set_ini_filename(None);
    context.io_mut().display_size = [400., 300.];
    context.io_mut().display_framebuffer_scale = [2., 2.];
    context.io_mut().delta_time = 1. / 60.;
    context.fonts().build_rgba32_texture();
    context.io_mut().app_focus_lost = true;
    let mut mouse = SyntheticMouseInput::default();
    frame(&mut context, &mut mouse);
    mouse
        .enqueue(MouseInput::Click {
            button: MouseButton::Left,
            position: Some([100., 140.]),
        })
        .unwrap();
    let frames: Vec<_> = (0..5).map(|_| frame(&mut context, &mut mouse)).collect();
    let pressed = frames
        .iter()
        .position(|f| f.2)
        .expect("press reaches imgui");
    let released = frames
        .iter()
        .position(|f| f.3)
        .expect("release reaches imgui");
    assert!(released > pressed, "a click spans distinct UI frames");
    assert_eq!(frames[pressed].0, [50., 70.]);
    assert_eq!(frames.iter().filter(|f| f.2).count(), 1);
    assert_eq!(frames.iter().filter(|f| f.3).count(), 1);
    assert!(!context.io().app_focus_lost);

    for position in [[200., 140.], [300., 140.]] {
        mouse
            .enqueue(MouseInput::Click {
                button: MouseButton::Left,
                position: Some(position),
            })
            .unwrap();
    }
    let frames: Vec<_> = (0..8).map(|_| frame(&mut context, &mut mouse)).collect();
    let clicks: Vec<_> = frames.iter().filter(|f| f.2).map(|f| f.0).collect();
    assert_eq!(clicks, [[100., 70.], [150., 70.]]);
    assert_eq!(frames.iter().filter(|f| f.3).count(), 2);

    mouse
        .enqueue(MouseInput::Button {
            button: MouseButton::Left,
            down: true,
            position: None,
        })
        .unwrap();
    mouse.enqueue(MouseInput::Move([300., 200.])).unwrap();
    let frames: Vec<_> = (0..5).map(|_| frame(&mut context, &mut mouse)).collect();
    assert_eq!(frames.last().unwrap().0, [150., 100.]);
    assert!(
        frames.last().unwrap().1,
        "drag remains held across movement"
    );
    mouse
        .enqueue(MouseInput::Scroll {
            delta: [1., -2.],
            position: None,
        })
        .unwrap();
    let frames: Vec<_> = (0..4).map(|_| frame(&mut context, &mut mouse)).collect();
    assert_eq!(frames.iter().map(|f| f.4[0]).sum::<f32>(), 1.);
    assert_eq!(frames.iter().map(|f| f.4[1]).sum::<f32>(), -2.);
    mouse
        .enqueue(MouseInput::Click {
            button: MouseButton::Right,
            position: Some([200., 200.]),
        })
        .unwrap();
    mouse.enqueue(MouseInput::Reset).unwrap();
    for _ in 0..4 {
        frame(&mut context, &mut mouse);
    }
    assert!(!context.io().mouse_down.iter().any(|down| *down));
    assert!(!mouse.active && mouse.pending.is_empty());
    context.io_mut().mouse_pos = [7., 8.];
    context.io_mut().mouse_down[1] = true;
    mouse.apply_frame(context.io_mut());
    assert_eq!(context.io().mouse_pos, [7., 8.]);
    assert!(
        context.io().mouse_down[1],
        "inactive synthetic input leaves real input alone"
    );
    assert!(mouse.enqueue(MouseInput::Move([f32::NAN, 1.])).is_err());
    assert!(
        mouse
            .enqueue(MouseInput::Scroll {
                delta: [0., f32::INFINITY],
                position: None,
            })
            .is_err()
    );
    assert!(mouse.pending.is_empty());
}
