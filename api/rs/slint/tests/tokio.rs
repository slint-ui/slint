// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[tokio::test]
async fn tokio_integration() {
    i_slint_backend_testing::init_integration_test_with_mock_time();

    let (sender, mut receiver) = tokio::sync::mpsc::channel(2);

    slint::spawn_local(tokio::task::unconstrained(async move {
        let mut count = 0;
        loop {
            if sender.send(count).await.is_err() {
                break;
            }
            count += 1;
        }
    }))
    .unwrap();

    slint::spawn_local(tokio::task::unconstrained(async move {
        loop {
            let count = receiver.recv().await.unwrap();
            if count > 1024 {
                slint::quit_event_loop().unwrap();
                break;
            }
        }
    }))
    .unwrap();

    slint::run_event_loop_until_quit().unwrap();
}
