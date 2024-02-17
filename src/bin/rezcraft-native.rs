fn main() {
    env_logger::init();

    pollster::block_on(rezcraft::do_run());
}
