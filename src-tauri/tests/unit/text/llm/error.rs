use super::*;

#[tokio::test]
async fn a_refused_connection_names_the_host_but_not_the_url() {
    let error = reqwest::Client::new()
        .post("http://127.0.0.1:1/v1/chat/completions?key=secret")
        .send()
        .await
        .expect_err("nothing listens on port 1");

    assert_eq!(
        request_failed(&error),
        "Couldn't reach 127.0.0.1:1. Is the server running?"
    );
}
