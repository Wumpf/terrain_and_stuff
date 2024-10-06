use crate::Application;

pub fn main_desktop() {
    env_logger::init_from_env(env_logger::Env::default().filter_or(
        env_logger::DEFAULT_FILTER_ENV,
        "warn,terrain_and_stuff=info",
    ));

    let mut application = pollster::block_on(Application::new());

    while application.window.is_open() {
        application.window.update();
        if application
            .window
            .is_key_pressed(minifb::Key::Escape, minifb::KeyRepeat::No)
        {
            return;
        }

        application.update();
        application.draw();
    }
}
