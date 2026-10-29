use next_gui_platform::desktop::OpenRequest;
fn main() {
    let path=std::env::args_os().nth(1).expect("owned absent path");
    assert!(!std::path::Path::new(&path).exists());
    let request=OpenRequest::path(std::path::Path::new(&path)).expect("worker starts");
    for _ in 0..500 {
        if let Some(result)=request.poll() {let error=result.expect_err("absent file cannot open");println!("Native missing-file failure reached caller: {error}");return;}
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("native launcher did not return");
}
