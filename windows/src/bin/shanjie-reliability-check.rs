use shanjie_windows::broker;
fn main() {
    assert_eq!(
        std::env::var("SHANJIE_TEST_PROFILE").as_deref(),
        Ok("1"),
        "Use an isolated test profile"
    );
    let args: Vec<_> = std::env::args().collect();
    let data = std::path::Path::new(&args[1]);
    match args[2].as_str() {
        "pipe" => broker::verification::pipe_deadlines(),
        "fault" => broker::verification::request_fault(data),
        "busy" => broker::verification::busy_learning(data),
        "ambiguous" => broker::verification::ambiguous(data),
        "capacity" => broker::verification::capacity(data),
        "restart" => broker::verification::restart(data),
        "restricted-identity" => broker::verification::restricted_identity(data,args[3].parse().unwrap()),
        "profile-root" => broker::verification::profile_root(),
        "log-failure" => broker::verification::log_failure(data),
        _ => panic!("Unknown isolated check"),
    }
}
