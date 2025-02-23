use crate::Application;

pub fn main_desktop() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().filter_or(
        env_logger::DEFAULT_FILTER_ENV,
        // Disable wgpu vulkan instance logging - it spams us with errors if we can't draw the frame at all.
        "warn,terrain_and_stuff=info,wgpu_hal::vulkan::instance=off",
    ));

    let mut application = pollster::block_on(Application::new())?;

    loop {
        application.window.update();
        if application
            .window
            .is_key_pressed(minifb::Key::Escape, minifb::KeyRepeat::No)
        {
            return Ok(());
        }

        // It's important to check openness after updating the window.
        // Otherwise, wgpu's surface might be invalid now.
        if !application.window.is_open() {
            return Ok(());
        }

        application.update();
        application.draw();
    }
}
