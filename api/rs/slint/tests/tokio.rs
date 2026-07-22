// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[test]
fn tokio_poll_with_compat() {
    i_slint_backend_testing::init_integration_test_with_mock_time();
    use std::io::Write;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let local_addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut stream = listener.incoming().next().unwrap().unwrap();
        stream.write("Hello World".as_bytes()).unwrap();
    });

    let slint_future = async move {
        for _ in 0..1000 {
            tokio::task::consume_budget().await;
        }

        use tokio::io::AsyncReadExt;
        let mut stream = tokio::net::TcpStream::connect(local_addr).await.unwrap();
        let mut data = Vec::new();
        stream.read_to_end(&mut data).await.unwrap();
        assert_eq!(data, "Hello World".as_bytes());
        slint::quit_event_loop().unwrap();
    };

    slint::spawn_local(async_compat::Compat::new(slint_future)).unwrap();

    slint::run_event_loop_until_quit().unwrap();

    server.join().unwrap();
}
